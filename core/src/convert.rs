use crate::{pipe::InnerPipe, prelude::*, render_helper::PureRender};

/// Trait for conversions type as a child of widget. The opposite of
/// `ChildFrom`.
///
/// You should not directly implement this trait. Instead, implement
/// `ChildFrom`.
///
/// It is similar to `Into` but with a const marker to automatically implement
/// all possible conversions without implementing conflicts.
pub trait RInto<T, K: ?Sized> {
  fn r_into(self) -> T;
}
/// Used to do value-to-value conversions while consuming the input value. It is
/// the reciprocal of `IntoChild`.
///
/// One should always prefer implementing `ChildFrom` over
/// `IntoChild`, because implementing `ChildFrom` will
/// automatically implement `IntoChild`.
pub trait RFrom<C, K: ?Sized> {
  fn r_from(from: C) -> Self;
}

// ---------- Child Conversion ---------

// into kind
pub struct IntoKind;
impl<C, T> RFrom<C, IntoKind> for T
where
  C: Into<T>,
{
  #[inline]
  fn r_from(from: C) -> Self { from.into() }
}

// template builder from
impl<Builder, C, K: ?Sized> RFrom<C, TmlKind<K>> for Builder
where
  Builder: TemplateBuilder + ComposeWithChild<C, K, Target = Builder>,
{
  fn r_from(from: C) -> Self { Builder::default().with_child(from) }
}

// Pair conversion

impl<'c, P, C, K> RFrom<Pair<P, C>, K> for Pair<P, Widget<'c>>
where
  C: IntoWidget<'c, K>,
  K: NotWidgetSelf
{
  fn r_from(from: Pair<P, C>) -> Self {
    let (parent, child) = from.unzip();
    Pair::new(parent, child.into_widget())
  }
}

impl<'c, W, C, K: ?Sized> RFrom<Pair<W, C>, K> for PairOf<'c, W>
where
  W: ComposeChild<'c, Child: RFrom<C, K>> + 'static,
{
  fn r_from(from: Pair<W, C>) -> Self {
    let (parent, child) = from.unzip();
    Self(FatObj::new(Pair::new(State::value(parent), child.r_into())))
  }
}

impl<'c, W, C, K: ?Sized> RFrom<Pair<State<W>, C>, K> for PairOf<'c, W>
where
  W: ComposeChild<'c, Child: RFrom<C, K>> + 'static,
{
  fn r_from(from: Pair<State<W>, C>) -> Self {
    let (parent, child) = from.unzip();
    Self(FatObj::new(Pair::new(parent, child.r_into())))
  }
}

impl<'c, W, C, K: ?Sized> RFrom<FatObj<Pair<State<W>, C>>, K> for PairOf<'c, W>
where
  W: ComposeChild<'c, Child: RFrom<C, K>> + 'static,
{
  fn r_from(from: FatObj<Pair<State<W>, C>>) -> Self {
    let pair = from.map(|p| {
      let (parent, child) = p.unzip();
      Pair::new(parent, child.r_into())
    });
    Self(pair)
  }
}

//  ----- Widget conversion ------
/// Marker type for resolving dual behavior ambiguity in pipe-based optional
/// widgets
///
/// Enables `MultiChild` to handle `Pipe<Option<impl IntoWidget>>` child
/// with clarity between:
///
/// 1. **Direct widget** - Treat the entire pipe as a single optional widget
/// 2. **Iterated widgets** - Process the pipe's optional value as successive
///    widget iterations
pub struct PipeOptionWidget<K: ?Sized>(PhantomData<fn() -> K>);

/// Marker for widgets converted from types not classified as `Widget` or
/// `PipeOptionWidget`
pub struct OtherWidget<K: ?Sized>(PhantomData<fn() -> K>);
pub(crate) trait NotWidgetSelf {}
impl<K: ?Sized> NotWidgetSelf for OtherWidget<K> {}
impl<K:?Sized> NotWidgetSelf for PipeOptionWidget<K> {}

// --- Compose Kind ---
impl<C: Compose + 'static> RFrom<C, OtherWidget<dyn Compose>> for Widget<'static> {
  fn r_from(widget: C) -> Self { Compose::compose(State::value(widget)) }
}

impl<W: StateWriter<Value: Compose + Sized>>
  RFrom<W, OtherWidget<dyn StateWriter<Value = &dyn Compose>>> for Widget<'static>
{
  fn r_from(widget: W) -> Self { Compose::compose(widget) }
}

// --- Render Kind ---

impl<R: Render + 'static> RFrom<R, OtherWidget<dyn Render>> for Widget<'static> {
  fn r_from(widget: R) -> Self { Widget::from_render(Box::new(PureRender(widget))) }
}

struct ReaderRender<T>(T);
impl<R: StateReader<Value: Render>> crate::render_helper::RenderProxy for ReaderRender<R> {
  #[inline(always)]
  fn proxy(&self) -> impl Deref<Target = impl Render + ?Sized> { self.0.read() }
}

macro_rules! impl_into_x_widget_for_state_reader {
  (<$($generics:ident $(: $bounds:ident)?),* > $ty:ty $(where $($t: tt)*)?) => {
    impl<$($generics $(:$bounds)?,)*> RFrom<$ty, OtherWidget<dyn Render>> for Widget<'static>
    $(where $($t)*)?
    {
      fn r_from(widget: $ty) -> Self {
        match widget.try_into_value() {
          Ok(value) => value.into_widget(),
          Err(s) => {
            ReaderRender(s).into_widget()
          },
        }
      }
    }
  };
}

macro_rules! impl_into_x_widget_for_state_watcher {
  (<$($generics:ident $(: $bounds:ident)?),* > $ty:ty $(where $($t: tt)*)?) => {
    impl<$($generics $(:$bounds)?,)*> RFrom<$ty, OtherWidget<dyn Render>> for Widget<'static>
    $(where $($t)*)?
    {
      fn r_from(widget: $ty) -> Self {
        match widget.try_into_value() {
          Ok(value) => value.into_widget(),
          Err(s) => {
            let modifies = s.raw_modifies();
            ReaderRender(s.clone_reader())
            .into_widget()
            .dirty_on(modifies, s.read().dirty_phase())
          },
        }
      }
    }
  };
}
impl_into_x_widget_for_state_reader!(<R: Render> Box<dyn StateReader<Value = R>>);
impl_into_x_widget_for_state_reader!(
  <O, M> MapReader<O, M>
  where MapReader<O, M>: StateReader<Value: Render + Sized>
);
impl_into_x_widget_for_state_watcher!(<R: Render> Stateful<R>);
impl_into_x_widget_for_state_watcher!(<R: Render> State<R>);
impl_into_x_widget_for_state_watcher!(
  <W, WM> MapWriter<W, WM>
  where MapWriter<W, WM>: StateWatcher<Value: Render + Sized>
);
impl_into_x_widget_for_state_watcher!(
  <O, M> SplittedWriter<O, M>
  where SplittedWriter<O, M>: StateWatcher<Value: Render + Sized>
);

// --- Function Kind ---
impl<'w, F, W, K> RFrom<F, OtherWidget<dyn FnOnce() -> K>> for Widget<'w>
where
  F: FnOnce() -> W + 'w,
  W: IntoWidget<'w, K> + 'w,
{
  #[inline]
  fn r_from(value: F) -> Self { Widget::from_fn(move |ctx| value().into_widget().call(ctx)) }
}

impl<'w, F, W, K> RFrom<FnWidget<W, F>, OtherWidget<dyn FnOnce() -> K>> for Widget<'w>
where
  F: FnOnce() -> W + 'w,
  W: IntoWidget<'w, K> + 'w,
{
  #[inline]
  fn r_from(value: FnWidget<W, F>) -> Self { value.0.into_widget() }
}

impl<F, W, K> RFrom<FnWidget<W, F>, dyn FnOnce() -> K> for GenWidget
where
  F: FnMut() -> W + 'static,
  W: IntoWidget<'static, K>,
{
  #[inline]
  fn r_from(value: FnWidget<W, F>) -> Self { GenWidget::from_fn_widget(value) }
}

impl<F, W, K> RFrom<F, dyn FnOnce() -> K> for GenWidget
where
  F: FnMut() -> W + 'static,
  W: IntoWidget<'static, K>,
{
  #[inline]
  fn r_from(value: F) -> Self { GenWidget::new(value) }
}

// --- FatObj Kind ---
impl<'w, T, K> RFrom<FatObj<T>, OtherWidget<FatObj<K>>> for Widget<'w>
where
  T: IntoWidget<'w, K>,
{
  fn r_from(value: FatObj<T>) -> Self { value.map(|w| w.into_widget()).compose() }
}

// ----  Pipe Kind ----

impl<P, K> RFrom<P, OtherWidget<dyn Pipe<Value = K>>> for Widget<'static>
where
  P: Pipe<Value: RInto<Widget<'static>, K>>,
{
  fn r_from(pipe: P) -> Self { pipe.build_single() }
}

impl<P, K, V> RFrom<P, PipeOptionWidget<K>> for Widget<'static>
where
  P: Pipe<Value = Option<V>>,
  V: RInto<Widget<'static>, K>,
{
  fn r_from(pipe: P) -> Self { pipe.build_single() }
}

// ---------- IntoChild implementation ----------------
impl<'a, C, T, K: ?Sized> RInto<C, K> for T
where
  C: RFrom<T, K>,
{
  fn r_into(self) -> C { C::r_from(self) }
}
