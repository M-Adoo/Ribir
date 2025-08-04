use std::convert::Infallible;

use rxrust::ops::box_it::{BoxOp, CloneableBoxOp};

use crate::prelude::*;

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

pub struct Watcher<R> {
  reader: R,
  modifies_observable: CloneableBoxOp<'static, ModifyInfo, Infallible>,
}

impl<R> Watcher<R> {
  pub fn new(
    reader: R, modifies_observable: CloneableBoxOp<'static, ModifyInfo, Infallible>,
  ) -> Self {
    Self { reader, modifies_observable }
  }
}

impl<R> From<Watcher<Reader<R>>> for Reader<R> {
  fn from(w: Watcher<Reader<R>>) -> Self { w.reader }
}

impl<R: StateReader> StateReader for Watcher<R> {
  type Value = R::Value;
  type Reader = R::Reader;

  #[inline]
  fn read(&self) -> ReadRef<Self::Value> { self.reader.read() }

  #[inline]
  fn clone_reader(&self) -> Self::Reader { self.reader.clone_reader() }

  #[inline]
  fn try_into_value(self) -> Result<Self::Value, Self>
  where
    Self::Value: Sized,
  {
    let Self { reader, modifies_observable } = self;
    reader
      .try_into_value()
      .map_err(|reader| Self { reader, modifies_observable })
  }
}

impl<R: StateReader> StateWatcher for Watcher<R> {
  type Watcher = Watcher<R::Reader>;

  fn into_reader(self) -> Result<Self::Reader, Self> { Err(self) }

  #[inline]
  fn clone_boxed_watcher(&self) -> Box<dyn StateWatcher<Value = Self::Value>> {
    Box::new(self.clone_watcher())
  }

  #[inline]
  fn raw_modifies(&self) -> CloneableBoxOp<'static, ModifyInfo, Infallible> {
    self.modifies_observable.clone()
  }

  fn clone_watcher(&self) -> Watcher<Self::Reader> {
    Watcher::new(self.clone_reader(), self.raw_modifies())
  }
}
