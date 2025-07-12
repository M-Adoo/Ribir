mod part_state;
mod prior_op;
mod stateful;
mod watcher;
mod writer;
use std::{convert::Infallible, ops::DerefMut};
pub mod state_cell;

pub use part_state::*;
pub use prior_op::*;
use rxrust::ops::box_it::{BoxOp, CloneableBoxOp};
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

/// The `StateReader` trait allows for reading, clone and map the state.
pub trait StateReader: 'static {
  /// The value type of this state.
  type Value: ?Sized;
  type Reader: StateReader<Value = Self::Value>
  where
    Self: Sized;

  /// Return a reference of this state.
  fn read(&self) -> ReadRef<'_, Self::Value>;

  /// Return a boxed reader of this state.
  fn clone_boxed_reader(&self) -> Box<dyn StateReader<Value = Self::Value>>;

  /// Return a cloned reader of this state.
  fn clone_reader(&self) -> Self::Reader
  where
    Self: Sized;
  /// Maps an reader to another by applying a function to a contained
  /// value. The return reader is just a shortcut to access part of the origin
  /// reader.
  ///
  /// Note, `MapReader` is a shortcut to access a portion of the original
  /// reader. It's assumed that the `map` function returns a part of the
  /// original data, not a cloned portion. Otherwise, the returned reader will
  /// not respond to state changes.
  #[inline]
  fn part_reader<U: ?Sized, F>(&self, map: F) -> PartReader<Self::Reader, F>
  where
    F: Fn(&Self::Value) -> PartRef<U> + Clone,
    Self: Sized,
  {
    PartReader { origin: self.clone_reader(), part_map: map }
  }

  /// try convert this state into the value, if there is no other share this
  /// state, otherwise return an error with self.
  fn try_into_value(self) -> Result<Self::Value, Self>
  where
    Self: Sized,
    Self::Value: Sized,
  {
    Err(self)
  }
}

pub trait StateWatcher: StateReader {
  type Watcher: StateWatcher<Value = Self::Value>
  where
    Self: Sized;

  /// Convert the writer to a reader if no other writers exist.
  fn into_reader(self) -> Result<Self::Reader, Self>
  where
    Self: Sized;

  /// Return a modifies `Rx` stream of the state, user can subscribe it to
  /// response the state changes.
  fn modifies(&self) -> BoxOp<'static, ModifyInfo, Infallible> {
    self
      .raw_modifies()
      .filter(|s| s.contains(ModifyEffect::DATA))
      .box_it()
  }

  /// Return a modifies `Rx` stream of the state, including all modifies. Use
  /// `modifies` instead if you only want to response the data changes.
  fn raw_modifies(&self) -> CloneableBoxOp<'static, ModifyInfo, Infallible>;

  /// Clone a boxed watcher that can be used to observe the modifies of the
  /// state.
  fn clone_boxed_watcher(&self) -> Box<dyn StateWatcher<Value = Self::Value>>;

  /// Clone a watcher that can be used to observe the modifies of the state.
  fn clone_watcher(&self) -> Self::Watcher
  where
    Self: Sized;

  /// Return a new watcher by applying a function to the contained value.
  fn part_watcher<U: ?Sized, F>(&self, map: F) -> Watcher<PartReader<Self::Reader, F>>
  where
    F: Fn(&Self::Value) -> PartRef<U> + Clone,
    Self: Sized,
  {
    let reader = self.part_reader(map);
    Watcher::new(reader, self.raw_modifies())
  }
}

pub struct WriteRef<'a, V: ?Sized> {
  value: ValueMutRef<'a, V>,
  info: &'a Sc<WriterInfo>,
  modify_effect: ModifyEffect,
  modified: bool,
  path: &'a PartialPath,
}

impl PartialId {
  pub fn new(str_id: CowArc<str>) -> Self { Self::from(str_id) }
}

impl<'a, V: ?Sized> WriteRef<'a, V> {
  pub fn silent(self) -> WriteRef<'a, V> {
    WriteRef {
      modify_effect: ModifyEffect::DATA,
      value: self.value.clone(),
      info: self.info,
      modified: false,
      path: self.path,
    }
  }

  pub fn shallow(self) -> WriteRef<'a, V> {
    WriteRef {
      modify_effect: ModifyEffect::FRAMEWORK,
      value: self.value.clone(),
      info: self.info,
      modified: false,
      path: self.path,
    }
  }

  pub fn map<U: ?Sized, M>(mut orig: WriteRef<'a, V>, part_map: M) -> WriteRef<'a, U>
  where
    M: Fn(&mut V) -> PartMut<U>,
  {
    let WriteRef { ref mut value, info, modify_effect: modify_scope, path, .. } = orig;

    let inner = part_map(value).inner;
    let value = ValueMutRef { inner, borrow: value.borrow.clone() };

    WriteRef { value, modified: false, modify_effect: modify_scope, info, path }
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
    let WriteRef { ref mut value, info, modify_effect: modify_scope, path: scopes, .. } = orig;
    match part_map(value) {
      Some(inner) => {
        let inner = inner.inner;
        let value = ValueMutRef { inner, borrow: value.borrow.clone() };
        Ok(WriteRef { value, modified: false, modify_effect: modify_scope, info, path: scopes })
      }
      None => Err(orig),
    }
  }

  /// Forget all modifies of this reference. So all the modifies occurred on
  /// this reference before this call will not be notified. Return true if there
  /// is any modifies on this reference.
  #[inline]
  pub fn forget_modifies(&mut self) -> bool { std::mem::replace(&mut self.modified, false) }
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
    self.modified = true;
    self.value.deref_mut()
  }
}

impl<'a, W: ?Sized> Drop for WriteRef<'a, W> {
  fn drop(&mut self) {
    let Self { info, modify_effect: modify_scope, modified, .. } = self;
    if !*modified {
      return;
    }

    let batched_modifies = &info.batched_modifies;
    if batched_modifies.get().is_empty() && !modify_scope.is_empty() {
      batched_modifies.set(*modify_scope);

      let info = info.clone();

      AppCtx::data_changed(self.path.clone(), info);
    } else {
      batched_modifies.set(*modify_scope | batched_modifies.get());
    }
  }
}

impl<V: ?Sized + 'static> StateReader for Box<dyn StateReader<Value = V>> {
  type Value = V;
  type Reader = Self;

  #[inline]
  fn read(&self) -> ReadRef<'_, V> { (**self).read() }

  #[inline]
  fn clone_boxed_reader(&self) -> Box<dyn StateReader<Value = Self::Value>> {
    (**self).clone_boxed_reader()
  }

  fn clone_reader(&self) -> Self::Reader { self.clone_boxed_reader() }
}

impl<V: ?Sized + 'static> StateReader for Box<dyn StateWatcher<Value = V>> {
  type Value = V;
  type Reader = Box<dyn StateReader<Value = V>>;

  #[inline]
  fn read(&self) -> ReadRef<'_, V> { (**self).read() }

  #[inline]
  fn clone_boxed_reader(&self) -> Box<dyn StateReader<Value = Self::Value>> {
    (**self).clone_boxed_reader()
  }

  #[inline]
  fn clone_reader(&self) -> Self::Reader { self.clone_boxed_reader() }
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

impl<V: ?Sized + 'static> StateReader for Box<dyn StateWriter<Value = V>> {
  type Value = V;
  type Reader = Box<dyn StateReader<Value = V>>;

  #[inline]
  fn read(&self) -> ReadRef<'_, V> { (**self).read() }

  #[inline]
  fn clone_boxed_reader(&self) -> Box<dyn StateReader<Value = Self::Value>> {
    (**self).clone_boxed_reader()
  }

  #[inline]
  fn clone_reader(&self) -> Self::Reader { self.clone_boxed_reader() }
}

impl<V: ?Sized + 'static> StateWatcher for Box<dyn StateWriter<Value = V>> {
  type Watcher = Box<dyn StateWatcher<Value = Self::Value>>;

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

impl<V: ?Sized + 'static> StateWriter for Box<dyn StateWriter<Value = V>> {
  #[inline]
  fn write(&self) -> WriteRef<Self::Value> { (**self).write() }
  #[inline]
  fn silent(&self) -> WriteRef<Self::Value> { (**self).silent() }
  #[inline]
  fn shallow(&self) -> WriteRef<Self::Value> { (**self).shallow() }
  #[inline]
  fn clone_boxed_writer(&self) -> Box<dyn StateWriter<Value = Self::Value>> {
    (**self).clone_boxed_writer()
  }
  #[inline]
  fn clone_writer(&self) -> Self { self.clone_boxed_writer() }

  fn part_writer<U: ?Sized + 'static, M>(&self, id: PartialId, part_map: M) -> PartWriter<Self, M>
  where
    M: Fn(&mut Self::Value) -> PartMut<U> + Clone + 'static,
    Self: Sized,
  {
    let mut path = self.scope_path().clone();
    if let Some(id) = id.0 {
      path.push(id);
    }

    PartWriter { origin: self.clone_writer(), part_map, path, include_partial: false }
  }

  #[inline]
  fn include_partial_writers(self, _: bool) -> Self {
    unimplemented!();
  }

  fn scope_path(&self) -> &PartialPath { (**self).scope_path() }
}

impl<T: Into<CowArc<str>>> From<T> for PartialId {
  #[inline]
  fn from(v: T) -> Self { Self(Some(v.into())) }
}
