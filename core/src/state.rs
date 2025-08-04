mod prior_op;
mod reader;
mod stateful;
mod watcher;
mod writer;
use std::{convert::Infallible, ops::DerefMut};
pub mod state_cell;

pub use prior_op::*;
pub use reader::*;
use rxrust::ops::box_it::CloneableBoxOp;
use smallvec::SmallVec;
pub use state_cell::*;
pub use stateful::*;
pub use watcher::*;
pub use writer::*;

use crate::prelude::*;

/// Identifier for a partial writer, supporting wildcard matching for parent
/// scope inheritance.
///
/// Use [`PartialId::any()`] to create a wildcard identifier that shares the
/// same scope as its parent writer.
pub struct PartialId(Option<CowArc<str>>);

/// Hierarchical path from root to current writer, represented as owned string
/// segments.
pub type PartialPath = SmallVec<[CowArc<str>; 1]>;

pub struct WriteRef<'a, V: ?Sized + 'a> {
  value: ValueMutRef<'a, V>,
  notify_guard: WriteRefNotifyGuard<'a>,
}

struct WriteRefNotifyGuard<'a> {
  info: &'a Sc<WriterInfo>,
  modify_effect: ModifyEffect,
  path: &'a PartialPath,
  modified: bool,
}

impl PartialId {
  pub fn new(str_id: CowArc<str>) -> Self { Self::from(str_id) }
}

impl<'a, V: ?Sized + 'a> WriteRef<'a, V> {
  fn new(
    value: ValueMutRef<'a, V>, info: &'a Sc<WriterInfo>, path: &'a PartialPath,
    modify_effect: ModifyEffect,
  ) -> Self {
    let notify_guard = WriteRefNotifyGuard { info, modify_effect, path, modified: false };
    WriteRef { value, notify_guard }
  }
  /// Converts to a silent write reference which notifies will be ignored by the
  /// framework.
  pub fn silent(self) -> WriteRef<'a, V> { self.with_modify_effect(ModifyEffect::DATA) }

  /// Converts to a shallow write reference. Modify across this reference will
  /// notify framework only. That means the modifies on shallow reference
  /// should only effect framework but not effect on data. eg. temporary to
  /// modify the state and then modifies it back to trigger the view update.
  /// Use it only if you know how a shallow reference works.
  pub fn shallow(self) -> WriteRef<'a, V> { self.with_modify_effect(ModifyEffect::FRAMEWORK) }

  pub fn map<U: ?Sized, M>(orig: WriteRef<'a, V>, part_map: M) -> WriteRef<'a, U>
  where
    M: Fn(&mut V) -> PartMut<U>,
  {
    let WriteRef { value, mut notify_guard } = orig;
    notify_guard.notify();
    let value = ValueMutRef::map(value, part_map);
    WriteRef { value, notify_guard }
  }

  /// Makes a new `WriteRef` for an optional component of the borrowed data. The
  /// original guard is returned as an `Err(..)` if the closure returns
  /// `None`.
  ///
  /// This is an associated function that needs to be used as
  /// `WriteRef::filter_map(...)`. A method would interfere with methods of the
  /// same name on `T` used through `Deref`.
  ///
  /// # Examples
  ///
  /// ```
  /// use ribir_core::prelude::*;
  ///
  /// let c = Stateful::new(vec![1, 2, 3]);
  /// let b1: WriteRef<Vec<u32>> = c.write();
  /// let b2: Result<WriteRef<u32>, _> =
  ///   WriteRef::filter_map(b1, |v| v.get_mut(1).map(PartMut::<u32>::new));
  /// assert_eq!(*b2.unwrap(), 2);
  /// ```
  pub fn filter_map<U: ?Sized, M>(
    mut orig: WriteRef<'a, V>, part_map: M,
  ) -> Result<WriteRef<'a, U>, Self>
  where
    M: Fn(&mut V) -> Option<PartMut<U>>,
  {
    match part_map(&mut orig.value).map(|v| v.inner) {
      Some(part) => {
        let WriteRef { value, mut notify_guard } = orig;
        notify_guard.notify();
        let ValueMutRef { inner, borrow, mut origin_store } = value;
        origin_store.add(inner);
        Ok(WriteRef { value: ValueMutRef { origin_store, inner: part, borrow }, notify_guard })
      }
      None => Err(orig),
    }
  }

  /// Forget all modifies of this reference. So all the modifies occurred on
  /// this reference before this call will not be notified. Return true if there
  /// is any modifies on this reference.
  #[inline]
  pub fn forget_modifies(&mut self) -> bool {
    std::mem::replace(&mut self.notify_guard.modified, false)
  }

  /// Internal helper to create a new WriteRef with specified modify effect
  fn with_modify_effect(mut self, modify_effect: ModifyEffect) -> WriteRef<'a, V> {
    self.notify_guard.notify();
    self.notify_guard.modify_effect = modify_effect;
    self
  }
}

impl<'a> WriteRefNotifyGuard<'a> {
  fn notify(&mut self) {
    let Self { info, modify_effect, modified, path } = self;
    if !*modified {
      return;
    }

    let batched_modifies = &info.batched_modifies;
    if batched_modifies.get().is_empty() && !modify_effect.is_empty() {
      batched_modifies.set(*modify_effect);
      AppCtx::data_changed(path.clone(), info.clone());
    } else {
      batched_modifies.set(*modify_effect | batched_modifies.get());
    }
    *modified = false;
  }
}
impl PartialId {
  /// A wildcard partial id, which means it equals to its parent scope.
  pub fn any() -> Self { Self(None) }
}

impl<'a, W: ?Sized> Deref for WriteRef<'a, W> {
  type Target = W;
  #[track_caller]
  #[inline]
  fn deref(&self) -> &Self::Target { self.value.deref() }
}

impl<'a, W: ?Sized> DerefMut for WriteRef<'a, W> {
  #[track_caller]
  #[inline]
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.notify_guard.modified = true;
    self.value.deref_mut()
  }
}

impl<'a> Drop for WriteRefNotifyGuard<'a> {
  fn drop(&mut self) { self.notify(); }
}

impl<V: ?Sized + 'static> StateReader for Box<dyn StateWatcher<Value = V>> {
  type Value = V;
  type Reader = Self;

  #[inline]
  fn read(&self) -> ReadRef<'_, V> { (**self).read() }

  #[inline]
  fn clone_reader(&self) -> Self::Reader { self.clone_boxed_watcher() }
}

impl<V: ?Sized + 'static> StateWatcher for Box<dyn StateWatcher<Value = V>> {
  type Watcher = Box<dyn StateWatcher<Value = V>>;

  #[inline]
  fn into_reader(self) -> Result<Self::Reader, Self> { Err(self) }

  #[inline]
  fn raw_modifies(&self) -> CloneableBoxOp<'static, ModifyInfo, Infallible> {
    (**self).raw_modifies()
  }

  #[inline]
  fn clone_boxed_watcher(&self) -> Box<dyn StateWatcher<Value = Self::Value>> {
    (**self).clone_boxed_watcher()
  }

  #[inline]
  fn clone_watcher(&self) -> Self::Watcher { self.clone_boxed_watcher() }
}

impl<T: Into<CowArc<str>>> From<T> for PartialId {
  #[inline]
  fn from(v: T) -> Self { Self(Some(v.into())) }
}
