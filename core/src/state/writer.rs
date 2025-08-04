use std::convert::Infallible;

use rxrust::ops::box_it::CloneableBoxOp;

use crate::prelude::*;

/// Enum to store both stateless and stateful object.
pub enum Writer<V> {
  Stateful(Stateful<V>),
  Part(PartWriter<V>),
}

pub struct PartWriter<V: ?Sized> {
  data: Box<dyn WriterPartial<Output = V>>,
  info: Sc<WriterInfo>,
  path: PartialPath,
  include_partial: bool,
}

impl<W> Writer<W> {
  pub fn value(value: W) -> Self { Writer::Stateful(Stateful::new(value)) }
}

impl<T: 'static> StateReader for Writer<T> {
  type Value = T;
  type Reader = Self;

  fn read(&self) -> ReadRef<'_, T> {
    match self {
      Writer::Stateful(w) => w.read(),
      Writer::Part(p) => p.read(),
    }
  }

  #[inline]
  fn clone_reader(&self) -> Self::Reader {
    // todo: clone only reader after refactored state reader
    self.clone_writer()
  }

  fn try_into_value(self) -> Result<Self::Value, Self> {
    match self {
      Writer::Stateful(w) => w.try_into_value().map_err(Writer::Stateful),
      Writer::Part(p) => p.try_into_value().map_err(Writer::Part),
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
    match self {
      Writer::Stateful(w) => w.raw_modifies(),
      Writer::Part(p) => p.raw_modifies(),
    }
  }

  #[inline]
  fn clone_watcher(&self) -> Watcher<Self::Reader> {
    Watcher::new(self.clone_reader(), self.raw_modifies())
  }
}

impl<V: 'static> Writer<V> {
  pub fn write(&self) -> WriteRef<V> {
    match self {
      Writer::Stateful(w) => w.write(),
      Writer::Part(p) => p.write(),
    }
  }

  pub fn silent(&self) -> WriteRef<V> {
    match self {
      Writer::Stateful(w) => w.silent(),
      Writer::Part(p) => p.silent(),
    }
  }

  pub fn shallow(&self) -> WriteRef<V> {
    match self {
      Writer::Stateful(w) => w.shallow(),
      Writer::Part(p) => p.shallow(),
    }
  }

  pub fn clone_writer(&self) -> Self {
    match self {
      Writer::Stateful(w) => Writer::Stateful(w.clone_writer()),
      Writer::Part(p) => Writer::Part(p.clone_writer()),
    }
  }

  /// Creates a child writer focused on a specific data segment identified by
  /// `id`.
  ///
  /// Establishes a parent-child hierarchy where:
  /// - `id` identifies a segment within the parent's data
  /// - `part_map` accesses the specific data segment
  /// - Parents control child modification propagation
  /// - Child is isolated from parent/sibling notifications
  ///
  /// # Parameters
  /// - `id`: Segment identifier (use `PartialId::any()` for wildcard)
  /// - `part_map`: Function mapping parent data to child's mutable data
  ///   reference
  pub fn part_writer<U: ?Sized + 'static>(
    &self, id: PartialId, part_map: impl Fn(&mut V) -> PartMut<U> + Clone + 'static,
  ) -> PartWriter<U> {
    match self.clone_writer() {
      Writer::Stateful(stateful) => stateful.part_writer(id, part_map),
      Writer::Part(part_writer) => part_writer.part_writer(id, part_map),
    }
  }

  /// Creates a wildcard child writer using a mapping function.
  ///
  /// Convenience method equivalent to `part_writer(PartialId::any(), part_map)`
  pub fn map_writer<U: ?Sized + 'static>(
    &self, part_map: impl Fn(&mut V) -> PartMut<U> + Clone + 'static,
  ) -> PartWriter<U> {
    self.part_writer(PartialId::any(), part_map)
  }

  #[inline]
  fn scope_path(&self) -> &PartialPath { wildcard_scope_path() }

  #[inline]
  fn include_partial_writers(self, include: bool) -> Self
  where
    Self: Sized,
  {
    match self {
      Writer::Stateful(w) => Writer::Stateful(w.include_partial_writers(include)),
      Writer::Part(p) => Writer::Part(p.include_partial_writers(include)),
    }
  }
}

impl<V: ?Sized + 'static> StateReader for PartWriter<V> {
  type Value = V;
  type Reader = Self;

  #[inline]
  fn read(&self) -> ReadRef<Self::Value> { self.data.read() }

  #[inline]
  fn clone_reader(&self) -> Self::Reader {
    // todo: clone only reader after refactored state reader
    self.clone_writer()
  }
}

impl<V: ?Sized + 'static> StateWatcher for PartWriter<V> {
  type Watcher = Watcher<Self::Reader>;

  fn into_reader(self) -> Result<Self::Reader, Self> {
    // todo: support it after flattened Reader
    return Err(self);
    // let Self { origin, part_map, path, include_partial } = self;
    // match origin.into_reader() {
    //   Ok(origin) => Ok(PartReader { origin, part_map:
    // WriterMapReaderFn(part_map) }),   Err(origin) => Err(Self { origin,
    // part_map, path, include_partial }), }
  }

  #[inline]
  fn clone_boxed_watcher(&self) -> Box<dyn StateWatcher<Value = Self::Value>> {
    Box::new(self.clone_watcher())
  }

  fn raw_modifies(&self) -> CloneableBoxOp<'static, ModifyInfo, Infallible> {
    let modifies = self
      .write()
      .notify_guard
      .info
      .notifier
      .raw_modifies();
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

impl<V: ?Sized + 'static> PartWriter<V> {
  pub fn write(&self) -> WriteRef<V> { self.write_ref(ModifyEffect::BOTH) }

  pub fn silent(&self) -> WriteRef<V> { self.write_ref(ModifyEffect::DATA) }

  pub fn shallow(&self) -> WriteRef<V> { self.write_ref(ModifyEffect::FRAMEWORK) }

  #[inline]
  pub fn clone_writer(&self) -> Self {
    PartWriter {
      data: self.data.clone_writer(),
      info: self.info.clone(),
      path: self.path.clone(),
      include_partial: self.include_partial,
    }
  }

  /// Creates a child writer focused on a specific data segment identified by
  /// `id`.
  ///
  /// Establishes a parent-child hierarchy where:
  /// - `id` identifies a segment within the parent's data
  /// - `part_map` accesses the specific data segment
  /// - Parents control child modification propagation
  /// - Child is isolated from parent/sibling notifications
  ///
  /// # Parameters
  /// - `id`: Segment identifier (use `PartialId::any()` for wildcard)
  /// - `part_map`: Function mapping parent data to child's mutable data
  ///   reference
  pub fn part_writer<U2: ?Sized>(
    &self, id: PartialId, part_map: impl Fn(&mut V) -> PartMut<U2> + Clone + 'static,
  ) -> PartWriter<U2> {
    let mut path = self.path.clone();
    if let Some(id) = id.0 {
      path.push(id);
    }

    PartWriter {
      data: Box::new(MapWriterPartData { origin: self.data.clone_writer(), partial: part_map }),
      info: self.info.clone(),
      path,
      include_partial: self.include_partial,
    }
  }

  /// Creates a wildcard child writer using a mapping function.
  ///
  /// Convenience method equivalent to `part_writer(PartialId::any(), part_map)`
  pub fn map_writer<U2: ?Sized>(
    &self, part_map: impl Fn(&mut V) -> PartMut<U2> + Clone + 'static,
  ) -> PartWriter<U2> {
    self.part_writer(PartialId::any(), part_map)
  }

  pub fn include_partial_writers(mut self, include: bool) -> Self {
    self.include_partial = include;
    self
  }

  fn scope_path(&self) -> &PartialPath { &self.path }

  fn write_ref(&self, effect: ModifyEffect) -> WriteRef<'_, V> {
    WriteRef::new(self.data.write(), &self.info, &self.path, effect)
  }
}

impl<T> RFrom<T, T> for Writer<T> {
  fn r_from(value: T) -> Self { Writer::value(value) }
}

impl<T> From<Stateful<T>> for Writer<T> {
  fn from(value: Stateful<T>) -> Self { Writer::Stateful(value) }
}

impl<V> From<PartWriter<V>> for Writer<V> {
  fn from(value: PartWriter<V>) -> Self { Writer::Part(value) }
}

pub(crate) struct PartData<V, F> {
  data: Sc<StateCell<V>>,
  partial: F,
}

pub(crate) struct MapWriterPartData<V: ?Sized, F> {
  origin: Box<dyn WriterPartial<Output = V>>,
  partial: F,
}

trait ReaderPartial {
  type Output: ?Sized;
  fn read(&self) -> ReadRef<Self::Output>;
  fn clone_reader(&self) -> Box<dyn ReaderPartial<Output = Self::Output>>;
}

trait WriterPartial: ReaderPartial {
  fn write(&self) -> ValueMutRef<Self::Output>;
  fn clone_writer(&self) -> Box<dyn WriterPartial<Output = Self::Output>>;
}

impl<V: 'static, U: ?Sized, F> ReaderPartial for PartData<V, F>
where
  F: Fn(&mut V) -> PartMut<U> + Clone + 'static,
{
  type Output = U;
  fn read(&self) -> ReadRef<U> {
    let value = self.data.read();
    ReadRef::mut_as_ref_map(value, &self.partial)
  }

  fn clone_reader(&self) -> Box<dyn ReaderPartial<Output = U>> {
    Box::new(PartData { data: self.data.clone(), partial: self.partial.clone() })
  }
}

impl<V: ?Sized + 'static, U: ?Sized, F> ReaderPartial for MapWriterPartData<V, F>
where
  F: Fn(&mut V) -> PartMut<U> + Clone + 'static,
{
  type Output = U;
  fn read(&self) -> ReadRef<U> {
    let value = self.origin.read();
    ReadRef::mut_as_ref_map(value, &self.partial)
  }
  fn clone_reader(&self) -> Box<dyn ReaderPartial<Output = U>> {
    Box::new(MapWriterPartData {
      origin: self.origin.clone_writer(),
      partial: self.partial.clone(),
    })
  }
}

impl<V: 'static, U: ?Sized, F> WriterPartial for PartData<V, F>
where
  F: Fn(&mut V) -> PartMut<U> + Clone + 'static,
{
  fn write(&self) -> ValueMutRef<U> {
    let value = self.data.write();
    ValueMutRef::map(value, &self.partial)
  }

  fn clone_writer(&self) -> Box<dyn WriterPartial<Output = U>> {
    Box::new(PartData { data: self.data.clone(), partial: self.partial.clone() })
  }
}

impl<V: ?Sized + 'static, U: ?Sized, F> WriterPartial for MapWriterPartData<V, F>
where
  F: Fn(&mut V) -> PartMut<U> + Clone + 'static,
{
  fn write(&self) -> ValueMutRef<U> {
    let value = self.origin.write();
    ValueMutRef::map(value, &self.partial)
  }
  fn clone_writer(&self) -> Box<dyn WriterPartial<Output = U>> {
    Box::new(MapWriterPartData {
      origin: self.origin.clone_writer(),
      partial: self.partial.clone(),
    })
  }
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
    fn compose(_: Writer<Self>) -> Widget<'static> { Void.into_widget() }
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
    fn compose_child(_: Writer<Self>, _: Self::Child) -> Widget<'c> { Void.into_widget() }
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
