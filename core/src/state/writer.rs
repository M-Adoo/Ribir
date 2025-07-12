use std::{cell::UnsafeCell, convert::Infallible};

use rxrust::ops::box_it::CloneableBoxOp;

use crate::prelude::*;

/// Enum to store both stateless and stateful object.
pub struct Writer<W>(UnsafeCell<InnerWriter<W>>);

pub struct PartWriter<W, WM> {
  pub(super) origin: W,
  pub(super) part_map: WM,
  pub(super) path: PartialPath,
  pub(super) include_partial: bool,
}

trait BoxedPartWriter<V: ?Sized> {
  fn boxed_read(&self) -> ReadRef<V>;
  fn boxed_raw_modifies(&self) -> CloneableBoxOp<'static, ModifyInfo, Infallible>;
  fn boxed_include_partial_writers(self: Box<Self>, include: bool) -> Box<dyn BoxedPartWriter<V>>;
  fn boxed_clone_writer(&self) -> Box<dyn BoxedPartWriter<V>>;
  fn boxed_write(&self) -> WriteRef<V>;
  fn boxed_silent(&self) -> WriteRef<V>;
  fn boxed_shallow(&self) -> WriteRef<V>;
}

enum InnerWriter<V> {
  Stateful(Stateful<V>),
  Part(Box<dyn BoxedPartWriter<V>>),
}

impl<W> Writer<W> {
  pub fn stateful(stateful: Stateful<W>) -> Self {
    Writer(UnsafeCell::new(InnerWriter::Stateful(stateful)))
  }

  pub fn value(value: W) -> Self { Writer::stateful(Stateful::new(value)) }

  pub fn part<S, F>(part: PartWriter<S, F>) -> Self
  where
    S: StateWriter,
    F: Fn(&mut S::Value) -> PartMut<W> + Clone + 'static,
  {
    Writer(UnsafeCell::new(InnerWriter::Part(Box::new(part))))
  }

  fn inner_ref(&self) -> &InnerWriter<W> {
    // Safety: we only use this method to get the inner state, and no way to get the
    // mutable reference of the inner state except the `as_stateful` method and the
    // `as_stateful` will check the inner borrow state.
    unsafe { &*self.0.get() }
  }
}

pub trait StateWriter: StateWatcher {
  /// Return a write reference of this state.
  fn write(&self) -> WriteRef<Self::Value>;
  /// Return a silent write reference which notifies will be ignored by the
  /// framework.
  fn silent(&self) -> WriteRef<Self::Value>;
  /// Return a shallow write reference. Modify across this reference will notify
  /// framework only. That means the modifies on shallow reference should only
  /// effect framework but not effect on data. eg. temporary to modify the
  /// state and then modifies it back to trigger the view update. Use it only
  /// if you know how a shallow reference works.
  fn shallow(&self) -> WriteRef<Self::Value>;

  /// Clone a boxed writer of this state.
  fn clone_boxed_writer(&self) -> Box<dyn StateWriter<Value = Self::Value>>;

  /// Clone a writer of this state.
  fn clone_writer(&self) -> Self
  where
    Self: Sized;

  /// Creates a child writer focused on a specific data segment identified by
  /// `id`.
  ///
  /// This establishes a parent-child hierarchy where:
  /// - The `id` identifies a segment within the parent writer's data
  /// - The `part_map` function accesses the specific data segment
  /// - Parents can control whether child modifications propagate upstream
  /// - Child writer will not be notified of parent modifications and siblings
  ///   notifications
  ///
  /// # Parameters
  /// - `id`: Identifies the data segment (use `PartialId::any()` for wildcard)
  /// - `part_map`: Function mapping parent data to child's data segment
  fn part_writer<V: ?Sized + 'static, M>(&self, id: PartialId, part_map: M) -> PartWriter<Self, M>
  where
    M: Fn(&mut Self::Value) -> PartMut<V> + Clone + 'static,
    Self: Sized;

  /// Configures whether modifications from partial writers should be included
  /// in notifications.
  ///
  /// Default: `false` (partial writer modifications are not included)
  ///
  /// # Example
  /// Consider a primary writer `P` with a partial writer `A` created via:
  /// ```ignore
  /// let partial_a = p.partial_writer("A".into(), ...);
  /// ```
  ///
  /// When watching `P`, this setting determines whether modifications to
  /// `partial_a` will appear in notifications about `P`.
  ///
  /// Change this setting not effects the already subscribed downstream.p
  fn include_partial_writers(self, include: bool) -> Self
  where
    Self: Sized;

  /// Temporary API, not use it.
  fn scope_path(&self) -> &PartialPath;
}

impl<T: 'static> StateReader for Writer<T> {
  type Value = T;
  type Reader = Self;

  fn read(&self) -> ReadRef<'_, T> {
    match self.inner_ref() {
      InnerWriter::Stateful(w) => w.read(),
      InnerWriter::Part(p) => p.boxed_read(),
    }
  }

  #[inline]
  fn clone_boxed_reader(&self) -> Box<dyn StateReader<Value = Self::Value>> {
    Box::new(self.clone_reader())
  }

  #[inline]
  fn clone_reader(&self) -> Self::Reader {
    // todo: clone only reader after refactored state reader
    self.clone_writer()
  }

  fn try_into_value(self) -> Result<Self::Value, Self> {
    match self.0.into_inner() {
      InnerWriter::Stateful(w) => w.try_into_value().map_err(Writer::stateful),
      InnerWriter::Part(p) => {
        // todo: support it after flattened PartWriter
        Err(Writer(UnsafeCell::new(InnerWriter::Part(p))))
      }
    }
  }
}

impl<T: 'static> StateWatcher for Writer<T> {
  type Watcher = Watcher<Self::Reader>;

  fn into_reader(self) -> Result<Self::Reader, Self>
  where
    Self: Sized,
  {
    // todo: support it after flattened PartWriter
    Err(self)
  }

  #[inline]
  fn clone_boxed_watcher(&self) -> Box<dyn StateWatcher<Value = Self::Value>> {
    Box::new(self.clone_watcher())
  }

  #[inline]
  fn raw_modifies(&self) -> CloneableBoxOp<'static, ModifyInfo, Infallible> {
    match self.inner_ref() {
      InnerWriter::Stateful(w) => w.raw_modifies(),
      InnerWriter::Part(p) => p.boxed_raw_modifies(),
    }
  }

  #[inline]
  fn clone_watcher(&self) -> Watcher<Self::Reader> {
    Watcher::new(self.clone_reader(), self.raw_modifies())
  }
}

impl<T: 'static> StateWriter for Writer<T> {
  fn write(&self) -> WriteRef<T> {
    match self.inner_ref() {
      InnerWriter::Stateful(w) => w.write(),
      InnerWriter::Part(p) => p.boxed_write(),
    }
  }

  fn silent(&self) -> WriteRef<T> {
    match self.inner_ref() {
      InnerWriter::Stateful(w) => w.silent(),
      InnerWriter::Part(p) => p.boxed_silent(),
    }
  }

  fn shallow(&self) -> WriteRef<T> {
    match self.inner_ref() {
      InnerWriter::Stateful(w) => w.shallow(),
      InnerWriter::Part(p) => p.boxed_shallow(),
    }
  }

  #[inline]
  fn clone_boxed_writer(&self) -> Box<dyn StateWriter<Value = Self::Value>> {
    Box::new(self.clone_writer())
  }

  fn clone_writer(&self) -> Self {
    match self.inner_ref() {
      InnerWriter::Stateful(w) => Writer::stateful(w.clone_writer()),
      InnerWriter::Part(p) => Writer(UnsafeCell::new(InnerWriter::Part(p.boxed_clone_writer()))),
    }
  }

  fn part_writer<V: ?Sized + 'static, M>(&self, id: PartialId, part_map: M) -> PartWriter<Self, M>
  where
    M: Fn(&mut Self::Value) -> PartMut<V> + Clone + 'static,
    Self: Sized,
  {
    let mut path = self.scope_path().clone();
    if let Some(id) = id.0 {
      path.push(id);
    }

    PartWriter { origin: self.clone_writer(), part_map, path, include_partial: false }
  }

  #[inline]
  fn scope_path(&self) -> &PartialPath { wildcard_scope_path() }

  #[inline]
  fn include_partial_writers(self, include: bool) -> Self
  where
    Self: Sized,
  {
    match self.0.into_inner() {
      InnerWriter::Stateful(w) => Writer::stateful(w.include_partial_writers(include)),
      InnerWriter::Part(p) => {
        Writer(UnsafeCell::new(InnerWriter::Part(p.boxed_include_partial_writers(include))))
      }
    }
  }
}

impl<V: ?Sized, S, M> StateReader for PartWriter<S, M>
where
  Self: 'static,
  S: StateWriter,
  M: Fn(&mut S::Value) -> PartMut<V> + Clone,
{
  type Value = V;
  type Reader = PartReader<S::Reader, WriterMapReaderFn<M>>;

  #[inline]
  fn read(&self) -> ReadRef<Self::Value> { self.boxed_read() }

  #[inline]
  fn clone_boxed_reader(&self) -> Box<dyn StateReader<Value = Self::Value>> {
    Box::new(self.clone_reader())
  }

  #[inline]
  fn clone_reader(&self) -> Self::Reader {
    PartReader {
      origin: self.origin.clone_reader(),
      part_map: WriterMapReaderFn(self.part_map.clone()),
    }
  }
}

impl<V: ?Sized, W, M> StateWatcher for PartWriter<W, M>
where
  Self: 'static,
  W: StateWriter,
  M: Fn(&mut W::Value) -> PartMut<V> + Clone,
{
  type Watcher = Watcher<Self::Reader>;

  fn into_reader(self) -> Result<Self::Reader, Self> {
    let Self { origin, part_map, path, include_partial } = self;
    match origin.into_reader() {
      Ok(origin) => Ok(PartReader { origin, part_map: WriterMapReaderFn(part_map) }),
      Err(origin) => Err(Self { origin, part_map, path, include_partial }),
    }
  }

  #[inline]
  fn clone_boxed_watcher(&self) -> Box<dyn StateWatcher<Value = Self::Value>> {
    Box::new(self.clone_watcher())
  }

  fn raw_modifies(&self) -> CloneableBoxOp<'static, ModifyInfo, Infallible> {
    let modifies = self.write().info.notifier.raw_modifies();
    let path = self.path.clone();
    let include_partial = self.include_partial;

    if !self.path.is_empty() {
      modifies
        .filter(move |info| info.path_matches(&path, include_partial))
        .box_it()
    } else {
      modifies
    }
  }

  #[inline]
  fn clone_watcher(&self) -> Watcher<Self::Reader> {
    Watcher::new(self.clone_reader(), self.raw_modifies())
  }
}

impl<V: ?Sized, W, M> StateWriter for PartWriter<W, M>
where
  Self: 'static,
  W: StateWriter,
  M: Fn(&mut W::Value) -> PartMut<V> + Clone,
{
  fn write(&self) -> WriteRef<Self::Value> {
    let mut w = WriteRef::map(self.origin.write(), &self.part_map);
    w.path = &self.path;
    w
  }

  fn silent(&self) -> WriteRef<Self::Value> {
    let mut w = WriteRef::map(self.origin.silent(), &self.part_map);
    w.path = &self.path;
    w
  }

  fn shallow(&self) -> WriteRef<Self::Value> {
    let mut w = WriteRef::map(self.origin.shallow(), &self.part_map);
    w.path = &self.path;
    w
  }

  #[inline]
  fn clone_boxed_writer(&self) -> Box<dyn StateWriter<Value = Self::Value>> {
    Box::new(self.clone_writer())
  }

  #[inline]
  fn clone_writer(&self) -> Self {
    PartWriter {
      origin: self.origin.clone_writer(),
      part_map: self.part_map.clone(),
      path: self.path.clone(),
      include_partial: self.include_partial,
    }
  }

  fn part_writer<U: ?Sized + 'static, F>(&self, id: PartialId, part_map: F) -> PartWriter<Self, F>
  where
    F: Fn(&mut Self::Value) -> PartMut<U> + Clone + 'static,
    Self: Sized,
  {
    let mut path = self.scope_path().clone();
    if let Some(id) = id.0 {
      path.push(id);
    }

    PartWriter { origin: self.clone_writer(), part_map, path, include_partial: false }
  }

  fn include_partial_writers(mut self, include: bool) -> Self {
    self.include_partial = include;
    self
  }

  fn scope_path(&self) -> &PartialPath { &self.path }
}

impl<U: ?Sized, S, M> BoxedPartWriter<U> for PartWriter<S, M>
where
  Self: 'static,
  S: StateWriter,
  M: Fn(&mut S::Value) -> PartMut<U> + Clone,
{
  #[inline]
  fn boxed_read(&self) -> ReadRef<U> { ReadRef::mut_as_ref_map(self.origin.read(), &self.part_map) }

  #[inline]
  fn boxed_raw_modifies(&self) -> CloneableBoxOp<'static, ModifyInfo, Infallible> {
    self.raw_modifies()
  }

  fn boxed_include_partial_writers(
    mut self: Box<Self>, include: bool,
  ) -> Box<dyn BoxedPartWriter<U>> {
    self.include_partial = include;
    self
  }

  fn boxed_clone_writer(&self) -> Box<dyn BoxedPartWriter<U>> { Box::new(self.clone_writer()) }

  #[inline]
  fn boxed_write(&self) -> WriteRef<U> { self.write() }

  #[inline]
  fn boxed_silent(&self) -> WriteRef<U> { self.silent() }

  #[inline]
  fn boxed_shallow(&self) -> WriteRef<U> { self.shallow() }
}

impl<T> RFrom<T, T> for Writer<T> {
  fn r_from(value: T) -> Self { Writer::value(value) }
}

impl<T> From<Stateful<T>> for Writer<T> {
  fn from(value: Stateful<T>) -> Self { Writer::stateful(value) }
}

impl<S, F, U> From<PartWriter<S, F>> for Writer<U>
where
  S: StateWriter,
  F: Fn(&mut S::Value) -> PartMut<U> + Clone + 'static,
{
  fn from(part: PartWriter<S, F>) -> Self { Writer::part(part) }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use super::*;
  use crate::reset_test_env;
  #[cfg(target_arch = "wasm32")]
  use crate::test_helper::wasm_bindgen_test;

  struct Origin {
    a: i32,
    b: i32,
  }

  #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
  #[test]
  fn map_same_with_origin() {
    reset_test_env!();

    let origin = Writer::value(Origin { a: 0, b: 0 });
    let map_state = origin.part_writer(PartialId::any(), |v| PartMut::new(&mut v.b));

    let track_origin = Sc::new(Cell::new(0));
    let track_map = Sc::new(Cell::new(0));

    let c_origin = track_origin.clone();
    origin.modifies().subscribe(move |_| {
      c_origin.set(c_origin.get() + 1);
    });

    let c_map = track_map.clone();
    map_state.modifies().subscribe(move |_| {
      c_map.set(c_map.get() + 1);
    });

    origin.write().a = 1;
    AppCtx::run_until_stalled();

    assert_eq!(track_origin.get(), 1);
    assert_eq!(track_map.get(), 1);

    *map_state.write() = 1;

    AppCtx::run_until_stalled();

    assert_eq!(track_origin.get(), 2);
    assert_eq!(track_map.get(), 2);
  }

  #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
  #[test]
  fn split_notify() {
    reset_test_env!();

    let origin = Writer::value(Origin { a: 0, b: 0 }).include_partial_writers(true);
    let split_a = origin.part_writer("a".into(), |v| PartMut::new(&mut v.a));
    let split_b = origin.part_writer("b".into(), |v| PartMut::new(&mut v.b));

    let track_origin = Sc::new(Cell::new(0));
    let track_split_a = Sc::new(Cell::new(0));
    let track_split_b = Sc::new(Cell::new(0));

    let c_origin = track_origin.clone();
    origin.modifies().subscribe(move |s| {
      c_origin.set(c_origin.get() + s.effect.bits());
    });

    let c_split_a = track_split_a.clone();
    split_a.modifies().subscribe(move |s| {
      c_split_a.set(c_split_a.get() + s.effect.bits());
    });

    let c_split_b = track_split_b.clone();
    split_b.modifies().subscribe(move |s| {
      c_split_b.set(c_split_b.get() + s.effect.bits());
    });

    *split_a.write() = 0;
    AppCtx::run_until_stalled();

    assert_eq!(track_origin.get(), ModifyEffect::BOTH.bits());
    assert_eq!(track_split_a.get(), ModifyEffect::BOTH.bits());
    assert_eq!(track_split_b.get(), 0);

    track_origin.set(0);
    track_split_a.set(0);

    *split_b.write() = 0;
    AppCtx::run_until_stalled();
    assert_eq!(track_origin.get(), ModifyEffect::BOTH.bits());
    assert_eq!(track_split_b.get(), ModifyEffect::BOTH.bits());
    assert_eq!(track_split_a.get(), 0);

    track_origin.set(0);
    track_split_b.set(0);

    origin.write().a = 0;
    AppCtx::run_until_stalled();
    assert_eq!(track_origin.get(), ModifyEffect::BOTH.bits());
    assert_eq!(track_split_b.get(), 0);
    assert_eq!(track_split_a.get(), 0);
  }

  struct C;

  impl Compose for C {
    fn compose(_: impl StateWriter<Value = Self>) -> Widget<'static> { Void.into_widget() }
  }

  #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
  #[test]
  fn state_writer_compose_builder() {
    reset_test_env!();

    let _state_compose_widget = fn_widget! {
      Writer::value(C)
    };

    let _sateful_compose_widget = fn_widget! {
      Stateful::new(C)
    };

    let _writer_compose_widget = fn_widget! {
      Stateful::new(C).clone_writer()
    };

    let _part_writer_compose_widget = fn_widget! {
      Stateful::new((C, 0))
        .part_writer(PartialId::any(), |v| PartMut::new(&mut v.0))
    };
    let _part_writer_compose_widget = fn_widget! {
      Stateful::new((C, 0))
        .part_writer("C".into(), |v| PartMut::new(&mut v.0))
    };
  }

  struct CC;
  impl<'c> ComposeChild<'c> for CC {
    type Child = Option<Widget<'c>>;
    fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> Widget<'c> {
      Void.into_widget()
    }
  }

  #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
  #[test]
  fn state_writer_compose_child_builder() {
    reset_test_env!();

    let _state_with_child = fn_widget! {
      let cc = Writer::value(CC);
      @(cc) { @{ Void } }
    };

    let _state_without_child = fn_widget! {
      Writer::value(CC)
    };

    let _stateful_with_child = fn_widget! {
      let cc = Stateful::new(CC);
      @(cc) { @{ Void } }
    };

    let _stateful_without_child = fn_widget! {
      Stateful::new(CC)
    };

    let _writer_with_child = fn_widget! {
      let cc = Stateful::new(CC).clone_writer();
      @(cc) { @{ Void } }
    };

    let _writer_without_child = fn_widget! {
      Stateful::new(CC).clone_writer()
    };

    let _part_writer_with_child = fn_widget! {
      let w = Stateful::new((CC, 0))
        .part_writer(PartialId::any(), |v| PartMut::new(&mut v.0));
      @(w) { @{ Void } }
    };

    let _part_writer_without_child = fn_widget! {
      Stateful::new((CC, 0))
        .part_writer(PartialId::any(), |v| PartMut::new(&mut v.0))
    };

    let _part_writer_with_child = fn_widget! {
      let w = Stateful::new((CC, 0))
        .part_writer("".into(), |v| PartMut::new(&mut v.0));
      @(w) { @{ Void } }
    };

    let _part_writer_without_child = fn_widget! {
      Stateful::new((CC, 0))
        .part_writer("".into(), |v| PartMut::new(&mut v.0))
    };
  }

  #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
  #[test]
  fn state_reader_builder() {
    reset_test_env!();

    let _state_render_widget = fn_widget! {
      Writer::value(Void)
    };

    let _stateful_render_widget = fn_widget! {
      Stateful::new(Void)
    };

    let _writer_render_widget = fn_widget! {
      Stateful::new(Void).clone_writer()
    };

    let _part_reader_render_widget = fn_widget! {
      Stateful::new((Void, 0)).part_reader(|v| PartRef::new(&v.0))
    };

    let _part_writer_render_widget = fn_widget! {
      Stateful::new((Void, 0))
        .part_writer(PartialId::any(), |v| PartMut::new(&mut v.0))
    };

    let _part_writer_render_widget = fn_widget! {
      Stateful::new((Void, 0))
        .part_writer("".into(), |v| PartMut::new(&mut v.0))
    };
  }

  #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
  #[test]
  fn trait_object_part_data() {
    reset_test_env!();
    let s = Writer::value(0);
    let m = s.part_writer("0".into(), |v| PartMut::new(v as &mut dyn Any));
    let v: ReadRef<dyn Any> = m.read();
    assert_eq!(*v.downcast_ref::<i32>().unwrap(), 0);

    let s = s.part_writer(PartialId::any(), |v| PartMut::new(v as &mut dyn Any));
    let v: ReadRef<dyn Any> = s.read();
    assert_eq!(*v.downcast_ref::<i32>().unwrap(), 0);
  }
}
