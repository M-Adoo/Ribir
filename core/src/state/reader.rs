use ribir_algo::Sc;

use crate::prelude::*;

/// The `StateReader` trait allows for reading, clone and map the state.
pub trait StateReader: 'static {
  /// The value type of this state.
  type Value: ?Sized;
  type Reader: StateReader<Value = Self::Value>
  where
    Self: Sized;

  /// Return a reference of this state.
  fn read(&self) -> ReadRef<'_, Self::Value>;

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

pub struct Reader<W>(pub(crate) Sc<StateCell<W>>);

/// A state reader that map a reader to another by applying a function on the
/// value. This reader is the same reader with the origin reader.
pub struct PartReader<S, F> {
  pub(super) origin: S,
  pub(super) part_map: F,
}

enum InnerReader<W> {
  Reader(Sc<StateCell<W>>),
  Part(Box<dyn BoxedReader<W>>),
}

trait BoxedReader<V> {
  fn boxed_read(&self) -> ReadRef<'_, V>;
  fn boxed_clone_reader(&self) -> Box<dyn BoxedReader<V>>;
}

impl<S, M, V: ?Sized> StateReader for PartReader<S, M>
where
  Self: 'static,
  S: StateReader,
  M: MapReaderFn<S::Value, Output = V>,
{
  type Value = V;
  type Reader = PartReader<S::Reader, M>;

  #[inline]
  fn read(&self) -> ReadRef<Self::Value> { self.part_map.call(self.origin.read()) }

  #[inline]
  fn clone_reader(&self) -> Self::Reader {
    PartReader { origin: self.origin.clone_reader(), part_map: self.part_map.clone() }
  }
}

trait MapReaderFn<Input: ?Sized>: Clone {
  type Output: ?Sized;
  fn call<'a>(&self, input: ReadRef<'a, Input>) -> ReadRef<'a, Self::Output>
  where
    Input: 'a;
}

impl<Input: ?Sized, Output: ?Sized, F> MapReaderFn<Input> for F
where
  F: Fn(&Input) -> PartRef<Output> + Clone,
{
  type Output = Output;
  fn call<'a>(&self, input: ReadRef<'a, Input>) -> ReadRef<'a, Self::Output>
  where
    Input: 'a,
  {
    ReadRef::map(input, self)
  }
}

impl<Input: ?Sized, Output: ?Sized, F> MapReaderFn<Input> for WriterMapReaderFn<F>
where
  F: Fn(&mut Input) -> PartMut<Output> + Clone,
{
  type Output = Output;
  fn call<'a>(&self, input: ReadRef<'a, Input>) -> ReadRef<'a, Self::Output>
  where
    Input: 'a,
  {
    ReadRef::mut_as_ref_map(input, &self.0)
  }
}

#[derive(Clone)]
pub struct WriterMapReaderFn<F>(pub(crate) F);

impl<W: 'static> StateReader for Reader<W> {
  type Value = W;
  type Reader = Self;

  #[inline]
  fn read(&self) -> ReadRef<'_, W> { self.0.read() }

  #[inline]
  fn clone_reader(&self) -> Self { Reader(self.0.clone()) }

  fn try_into_value(self) -> Result<Self::Value, Self> {
    if self.0.ref_count() == 1 {
      // SAFETY: `self.0.ref_count() == 1` guarantees unique access.
      let data = unsafe { Sc::try_unwrap(self.0).unwrap_unchecked() };
      Ok(data.into_inner())
    } else {
      Err(self)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{prelude::*, test_helper::*};

  #[test]
  fn isolated_writer() {
    reset_test_env!();

    let pair = Stateful::new((1., true));
    let first = pair.part_writer("1.".into(), |v| PartMut::new(&mut v.0));
    let second = pair.part_writer("2.".into(), |v| PartMut::new(&mut v.1));
    let (notifies, w_notifies) = split_value(vec![]);

    watch!(*$read(pair)).subscribe({
      let w_notifies = w_notifies.clone_writer();
      move |_| w_notifies.write().push("pair")
    });
    watch!(*$read(first)).subscribe({
      let w_notifies = w_notifies.clone_writer();
      move |_| w_notifies.write().push("first")
    });
    watch!(*$read(second)).subscribe({
      let w_notifies = w_notifies.clone_writer();
      move |_| w_notifies.write().push("second")
    });

    assert_eq!(&*notifies.read(), &["pair", "first", "second"]);
    *first.write() = 2.;
    AppCtx::run_until_stalled();
    assert_eq!(&*notifies.read(), &["pair", "first", "second", "first"]);
    *second.write() = false;
    AppCtx::run_until_stalled();
    assert_eq!(&*notifies.read(), &["pair", "first", "second", "first", "second"]);
    *pair.write() = (3., false);
    AppCtx::run_until_stalled();
    assert_eq!(&*notifies.read(), &["pair", "first", "second", "first", "second", "pair"]);
  }

  #[test]
  fn test_many() {
    for _ in 0..10 {
      isolated_writer();
    }
  }
}
