use super::*;

/// A state reader that map a reader to another by applying a function on the
/// value. This reader is the same reader with the origin reader.
pub struct PartReader<S, F> {
  pub(super) origin: S,
  pub(super) part_map: F,
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
  fn clone_boxed_reader(&self) -> Box<dyn StateReader<Value = Self::Value>> {
    Box::new(self.clone_reader())
  }

  #[inline]
  fn clone_reader(&self) -> Self::Reader {
    PartReader { origin: self.origin.clone_reader(), part_map: self.part_map.clone() }
  }
}

trait MapReaderFn<Input: ?Sized>: Clone {
  type Output: ?Sized;
  fn call<'a>(&self, input: ReadRef<'a, Input>) -> ReadRef<'a, Self::Output>;
}

impl<Input: ?Sized, Output: ?Sized, F> MapReaderFn<Input> for F
where
  F: Fn(&Input) -> PartRef<Output> + Clone,
{
  type Output = Output;
  fn call<'a>(&self, input: ReadRef<'a, Input>) -> ReadRef<'a, Self::Output> {
    ReadRef::map(input, self)
  }
}

impl<Input: ?Sized, Output: ?Sized, F> MapReaderFn<Input> for WriterMapReaderFn<F>
where
  F: Fn(&mut Input) -> PartMut<Output> + Clone,
{
  type Output = Output;
  fn call<'a>(&self, input: ReadRef<'a, Input>) -> ReadRef<'a, Self::Output> {
    ReadRef::mut_as_ref_map(input, &self.0)
  }
}

#[derive(Clone)]
pub struct WriterMapReaderFn<F>(pub(crate) F);

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
