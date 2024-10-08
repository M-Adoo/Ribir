use super::*;
use crate::widget::*;

/// A writer splitted writer from another writer, and has its own notifier.
pub struct SplittedWriter<O, W> {
  origin: O,
  splitter: W,
  info: Sc<WriterInfo>,
}

impl<O, W> Drop for SplittedWriter<O, W> {
  fn drop(&mut self) { self.info.dec_writer() }
}

impl<V: ?Sized, O, W> StateReader for SplittedWriter<O, W>
where
  Self: 'static,
  O: StateWriter,
  W: Fn(&mut O::Value) -> PartData<V> + Clone,
{
  type Value = V;
  type OriginReader = O;
  type Reader = MapWriterAsReader<O::Reader, W>;

  #[track_caller]
  fn read(&self) -> ReadRef<Self::Value> {
    ReadRef::mut_as_ref_map(self.origin.read(), &self.splitter)
  }

  #[inline]
  fn clone_reader(&self) -> Self::Reader {
    MapWriterAsReader { origin: self.origin.clone_reader(), part_map: self.splitter.clone() }
  }

  #[inline]
  fn origin_reader(&self) -> &Self::OriginReader { &self.origin }

  #[inline]
  fn try_into_value(self) -> Result<Self::Value, Self>
  where
    Self::Value: Sized,
  {
    Err(self)
  }
}

impl<V: ?Sized, O, W> StateWatcher for SplittedWriter<O, W>
where
  Self: 'static,
  O: StateWriter,
  W: Fn(&mut O::Value) -> PartData<V> + Clone,
{
  fn raw_modifies(&self) -> CloneableBoxOp<'static, ModifyScope, std::convert::Infallible> {
    self.info.notifier.raw_modifies().box_it()
  }
}

impl<V: ?Sized, O, W> StateWriter for SplittedWriter<O, W>
where
  Self: 'static,
  O: StateWriter,
  W: Fn(&mut O::Value) -> PartData<V> + Clone,
{
  type Writer = SplittedWriter<O::Writer, W>;
  type OriginWriter = O;

  fn into_reader(self) -> Result<Self::Reader, Self> {
    if self.info.writer_count.get() == 1 { Ok(self.clone_reader()) } else { Err(self) }
  }

  #[inline]
  fn write(&self) -> WriteRef<Self::Value> { self.split_ref(self.origin.write()) }

  #[inline]
  fn silent(&self) -> WriteRef<Self::Value> { self.split_ref(self.origin.silent()) }

  #[inline]
  fn shallow(&self) -> WriteRef<Self::Value> { self.split_ref(self.origin.shallow()) }

  fn clone_writer(&self) -> Self::Writer {
    self.info.inc_writer();
    SplittedWriter {
      origin: self.origin.clone_writer(),
      splitter: self.splitter.clone(),
      info: self.info.clone(),
    }
  }

  #[inline]
  fn origin_writer(&self) -> &Self::OriginWriter { &self.origin }
}

impl<V: ?Sized, O, W> SplittedWriter<O, W>
where
  O: StateWriter,
  W: Fn(&mut O::Value) -> PartData<V> + Clone,
{
  pub(super) fn new(origin: O, mut_map: W) -> Self {
    Self { origin, splitter: mut_map, info: Sc::new(WriterInfo::new()) }
  }

  #[track_caller]
  fn split_ref<'a>(&'a self, mut orig: WriteRef<'a, O::Value>) -> WriteRef<'a, V> {
    let modify_scope = orig.modify_scope;

    // the origin mark as a silent write, because split writer not effect the origin
    // state in ribir framework level. But keep notify in the data level.
    assert!(!orig.modified);
    orig.modify_scope.remove(ModifyScope::FRAMEWORK);
    orig.modified = true;
    let value =
      ValueMutRef { inner: (self.splitter)(&mut orig.value), borrow: orig.value.borrow.clone() };

    WriteRef { value, modified: false, modify_scope, info: &self.info }
  }
}

impl<'w, S, F> IntoWidgetStrict<'w, RENDER> for SplittedWriter<S, F>
where
  Self: StateWriter<Value: Render + Sized> + 'w,
{
  fn into_widget_strict(self) -> Widget<'w> { WriterRender(self).into_widget() }
}

impl<S, F> IntoWidgetStrict<'static, COMPOSE> for SplittedWriter<S, F>
where
  Self: StateWriter + 'static,
  <Self as StateReader>::Value: Compose,
{
  fn into_widget_strict(self) -> Widget<'static> { Compose::compose(self) }
}
