#[doc(hidden)]
pub use std::{
  any::{Any, TypeId},
  marker::PhantomData,
  ops::Deref,
};
use std::{cell::RefCell, convert::Infallible};

use ops::box_it::CloneableBoxOp;
use ribir_algo::Sc;
use widget_id::RenderQueryable;

pub(crate) use crate::widget_tree::*;
use crate::{context::*, pipe::InnerPipe, prelude::*, render_helper::PureRender};
pub trait Compose {
  /// Describes the part of the user interface represented by this widget.
  /// Called by framework, should never directly call it.
  fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static>
  where
    Self: Sized;
}

pub struct HitTest {
  pub hit: bool,
  pub can_hit_child: bool,
}

/// RenderWidget is a widget which want to paint something or do a layout to
/// calc itself size and update children positions.
pub trait Render: 'static {
  /// Do the work of computing the layout for this widget, and return the
  /// size it need.
  ///
  /// In implementing this function, You are responsible for calling every
  /// children's perform_layout across the `LayoutCtx`
  ///
  /// ## Guidelines for implementing this method
  ///
  /// - The clamp should restrict the size to always fall within the specified
  ///   range.
  /// - Avoid returning infinity or NaN size, as this could result in a crash.
  ///   If your size calculation is dependent on the `clamp.max`, you might want
  ///   to consider using [`LayoutCtx::fixed_max`].
  /// - Parent has responsibility to call the children's perform_layout, and
  ///   update the children's position. If the children position is not updated
  ///   that will set to zero.
  fn perform_layout(&self, clamp: BoxClamp, ctx: &mut LayoutCtx) -> Size;

  /// Draw the widget on the paint device using `PaintingCtx::painter` within
  /// its own coordinate system. This method should not handle painting of
  /// children; the framework will handle painting of children individually. The
  /// framework ensures that the parent is always painted before its children.
  fn paint(&self, _: &mut PaintingCtx) {}

  /// Whether the child nodes' size affect its size.
  fn size_affected_by_child(&self) -> bool { true }

  /// Verify if the provided position is within this widget and return whether
  /// its child can be hit if the widget itself is not hit.
  fn hit_test(&self, ctx: &mut HitTestCtx, pos: Point) -> HitTest {
    let hit = ctx.box_hit_test(pos);
    // If the widget is affected by child, indicating it is not a
    // fixed-size container, we permit the child to receive hits even if it
    // extends beyond its parent boundaries.
    HitTest { hit, can_hit_child: hit || self.size_affected_by_child() }
  }

  /// By default, this function returns a `Layout` phase to indicate that the
  /// widget should be marked as dirty when modified. When the layout phase is
  /// marked as dirty, the paint phase will also be affected.
  fn dirty_phase(&self) -> DirtyPhase { DirtyPhase::Layout }

  /// Return a transform to map the coordinate to parent coordinate.
  fn get_transform(&self) -> Option<Transform> { None }

  /// Computes the visual bounding box of the widget relative to self.
  /// The method is called by framework after the layout is done.
  /// Usually if you paint something, you should return the bounding box of the
  /// paint. Default implementation will return None which means the
  /// current widget will not be rendered.
  ///
  ///
  /// Parameters:
  ///
  /// * `ctx`: The VisualCtx.
  ///
  /// Returns:
  ///
  /// The visual bounding box of the widget.
  #[allow(unused_variables)]
  fn visual_box(&self, ctx: &mut VisualCtx) -> Option<Rect> { None }
}

/// The common type of all widget can convert to.
pub struct Widget<'w>(InnerWidget<'w>);
pub(crate) struct InnerWidget<'w>(Box<dyn FnOnce(&mut BuildCtx) -> WidgetId + 'w>);

/// A trait for converting any widget into a `Widget` type.
///
/// Automatically implemented by the framework for types implementing
/// `Into<XWidget<W, K>>`. Direct implementations are not recommended.
pub trait IntoWidget<'a, K> {
  fn into_widget(self) -> Widget<'a>;
}

/// Marker trait for widget kind identification to assist framework type
/// conversions
pub(crate) trait WidgetKind {}

/// Marker for widgets converted from types not classified as `Widget` or
/// `PipeOptionWidget`
pub struct OtherWidget<K: ?Sized>(PhantomData<fn() -> K>);

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

// Consolidated WidgetKind implementations
impl WidgetKind for Widget<'_> {}
impl<K: ?Sized> WidgetKind for OtherWidget<K> {}
impl<K: ?Sized> WidgetKind for PipeOptionWidget<K> {}

/// A boxed function widget that can be called multiple times to regenerate
/// widget.
#[derive(Clone)]
pub struct GenWidget(InnerGenWidget);
type InnerGenWidget = Sc<RefCell<Box<dyn FnMut() -> Widget<'static>>>>;

pub struct FnWidget<W, F: FnOnce() -> W>(F);
pub type BoxFnWidget<'w> = Box<dyn FnOnce() -> Widget<'w> + 'w>;

impl<W, F> FnWidget<W, F>
where
  F: FnOnce() -> W,
{
  pub fn new<'w, K>(f: F) -> Self
  where
    W: IntoWidget<'w, K>,
  {
    Self(f)
  }

  pub fn into_inner(self) -> F { self.0 }

  pub fn call(self) -> W { (self.0)() }

  pub fn boxed<'w, K>(self) -> BoxFnWidget<'w>
  where
    W: IntoWidget<'w, K> + 'w,
    F: 'w,
  {
    Box::new(move || self.call().into_widget())
  }
}

impl GenWidget {
  pub fn new<W, K>(mut f: impl FnMut() -> W + 'static) -> Self
  where
    W: IntoWidget<'static, K>,
  {
    Self(Sc::new(RefCell::new(Box::new(move || f().into_widget()))))
  }

  pub fn from_fn_widget<F, W, K>(f: FnWidget<W, F>) -> Self
  where
    F: FnMut() -> W + 'static,
    W: IntoWidget<'static, K>,
  {
    Self::new(f.into_inner())
  }

  pub fn gen_widget(&self) -> Widget<'static> { self.0.borrow_mut()() }
}

impl<W: ComposeChild<'static, Child = Option<C>>, C> Compose for W {
  fn compose(this: impl StateWriter<Value = Self>) -> Widget<'static> {
    ComposeChild::compose_child(this, None)
  }
}

impl<'w> Widget<'w> {
  /// Invoke a function when the root node of the widget is built, passing its
  /// ID and build context as parameters.
  pub fn on_build(self, f: impl FnOnce(WidgetId) + 'w) -> Self {
    Widget::from_fn(move |ctx| {
      let id = self.call(ctx);
      f(id);
      id
    })
  }

  /// Subscribe to the modified `upstream` to mark the widget as dirty when the
  /// `upstream` emits a modify event containing `ModifyScope::FRAMEWORK`.
  ///
  /// # Panic
  /// This method only works within a build process; otherwise, it will
  /// result in a panic.
  pub fn dirty_on(
    self, upstream: CloneableBoxOp<'static, ModifyScope, Infallible>, dirty: DirtyPhase,
  ) -> Self {
    let track = TrackWidgetId::default();
    let id = track.track_id();

    let tree = BuildCtx::get_mut().tree_mut();
    let marker = tree.dirty_marker();
    let h = upstream
      .filter(|b| b.contains(ModifyScope::FRAMEWORK))
      .subscribe(move |_| {
        if let Some(id) = id.get() {
          marker.mark(id, dirty);
        }
      })
      .unsubscribe_when_dropped();

    track
      .with_child(self)
      .into_widget()
      .attach_anonymous_data(h)
  }

  pub(crate) fn from_render(r: Box<dyn RenderQueryable>) -> Widget<'static> {
    Widget::from_fn(|_| BuildCtx::get_mut().tree_mut().alloc_node(r))
  }

  /// Attach anonymous data to a widget and user can't query it.
  pub fn attach_anonymous_data(self, data: impl Any) -> Self {
    self.on_build(|id| id.attach_anonymous_data(data, BuildCtx::get_mut().tree_mut()))
  }

  pub fn attach_data(self, data: Box<dyn Query>) -> Self {
    self.on_build(|id| id.attach_data(data, BuildCtx::get_mut().tree_mut()))
  }

  /// Attach a state to a widget and try to unwrap it before attaching.
  ///
  /// User can query the state or its value type.
  pub fn try_unwrap_state_and_attach<D: Any>(
    self, data: impl StateWriter<Value = D> + 'static,
  ) -> Self {
    let data: Box<dyn Query> = match data.try_into_value() {
      Ok(data) => Box::new(Queryable(data)),
      Err(data) => Box::new(data),
    };
    self.attach_data(data)
  }

  /// Convert an ID back to a widget.
  ///
  /// # Note
  ///
  /// It's important to remember that we construct the tree lazily. In most
  /// cases, you should avoid using this method to create a widget unless you
  /// are certain that the entire logic is suitable for creating this widget
  /// from an ID.
  pub(crate) fn from_id(id: WidgetId) -> Widget<'static> { Widget::from_fn(move |_| id) }

  pub(crate) fn new(parent: Widget<'w>, children: Vec<Widget<'w>>) -> Widget<'w> {
    Widget::from_fn(move |ctx| ctx.build_parent(parent, children))
  }

  pub(crate) fn from_fn(f: impl FnOnce(&mut BuildCtx) -> WidgetId + 'w) -> Widget<'w> {
    Widget(InnerWidget(Box::new(f)))
  }

  pub(crate) fn call(self, ctx: &mut BuildCtx) -> WidgetId { (self.0.0)(ctx) }
}

/// XWidget organize widgets as two categories: `ConvertFrom` and
/// `KeepOriginal`.
///
/// - `ConvertFrom` means this `XWidget` created from a not `Widget`, and the
/// generic type `K` should hints the kind of the original widget.
///
/// - `KeepOriginal` means this `XWidget` created from a `Widget`, doesn't do
/// anything conversion.
///
/// Keep the kind information just help framework to do some type conversion
/// easier and can do some type checking.
pub(crate) struct XWidget<'a, K: WidgetKind> {
  pub(crate) widget: Widget<'a>,
  _kind: PhantomData<K>,
}

impl<'a, K: WidgetKind> XWidget<'a, K> {
  #[inline]
  pub fn new(widget: Widget<'a>) -> Self { Self { widget, _kind: PhantomData } }
}

// --- Compose Kind ---
impl<C: Compose + 'static> From<C> for XWidget<'static, OtherWidget<dyn Compose>> {
  fn from(widget: C) -> Self { Self::new(Compose::compose(State::value(widget))) }
}

impl<W: StateWriter<Value: Compose + Sized>> From<W>
  for XWidget<'static, OtherWidget<dyn StateWriter<Value = &dyn Compose>>>
{
  fn from(widget: W) -> Self { Self::new(Compose::compose(widget)) }
}

// --- Render Kind ---

impl<R: Render + 'static> From<R> for XWidget<'static, OtherWidget<dyn Render>> {
  fn from(widget: R) -> Self { Self::new(Widget::from_render(Box::new(PureRender(widget)))) }
}

struct ReaderRender<T>(T);
impl<R: StateReader<Value: Render>> crate::render_helper::RenderProxy for ReaderRender<R> {
  #[inline(always)]
  fn proxy(&self) -> impl Deref<Target = impl Render + ?Sized> { self.0.read() }
}

macro_rules! impl_into_x_widget_for_state_reader {
  (<$($generics:ident $(: $bounds:ident)?),* > $ty:ty $(where $($t: tt)*)?) => {
    impl<$($generics $(:$bounds)?,)*> From<$ty> for XWidget<'static, OtherWidget<dyn Render>>
    $(where $($t)*)?
    {
      fn from(widget: $ty) -> Self {
        let w = match widget.try_into_value() {
          Ok(value) => value.into_widget(),
          Err(s) => ReaderRender(s).into_widget(),
        };
        Self::new(w)
      }
    }
  };
}
impl_into_x_widget_for_state_reader!(<R: Render> Box<dyn StateReader<Value = R>>);
impl_into_x_widget_for_state_reader!(<R: Render> Stateful<R>);
impl_into_x_widget_for_state_reader!(<R: Render> State<R>);
impl_into_x_widget_for_state_reader!(
  <W, WM> MapWriter<W, WM>
  where MapWriter<W, WM>: StateReader<Value: Render + Sized>
);
impl_into_x_widget_for_state_reader!(
  <O, M> SplittedWriter<O, M>
  where SplittedWriter<O, M>: StateReader<Value: Render + Sized>
);
impl_into_x_widget_for_state_reader!(
  <O, M> MapReader<O, M>
  where MapReader<O, M>: StateReader<Value: Render + Sized>
);

// --- Function Kind ---
impl<'w, F, W, K> From<F> for XWidget<'w, OtherWidget<dyn FnOnce() -> K>>
where
  F: FnOnce() -> W + 'w,
  W: IntoWidget<'w, K> + 'w,
{
  #[inline]
  fn from(value: F) -> Self {
    Self::new(Widget::from_fn(move |ctx| value().into_widget().call(ctx)))
  }
}

impl<'w, F, W, K> From<FnWidget<W, F>> for XWidget<'w, OtherWidget<dyn FnOnce() -> K>>
where
  F: FnOnce() -> W + 'w,
  W: IntoWidget<'w, K> + 'w,
{
  #[inline]
  fn from(value: FnWidget<W, F>) -> Self { value.0.into() }
}

impl From<GenWidget> for XWidget<'static, OtherWidget<GenWidget>> {
  fn from(widget: GenWidget) -> Self {
    let w = FnWidget::new(move || widget.gen_widget()).into_widget();
    Self::new(w)
  }
}

// --- FatObj Kind ---
impl<'w, T, K> From<FatObj<T>> for XWidget<'w, OtherWidget<FatObj<K>>>
where
  T: IntoWidget<'w, K>,
{
  fn from(value: FatObj<T>) -> Self {
    let w = value.map(|w| w.into_widget()).compose();
    XWidget::<OtherWidget<_>>::new(w)
  }
}

// ----  Pipe Kind ----

impl<P, K: WidgetKind> From<P> for XWidget<'static, OtherWidget<dyn Pipe<Value = K>>>
where
  P: Pipe<Value: Into<XWidget<'static, K>>>,
{
  fn from(pipe: P) -> Self { XWidget::new(InnerPipe::build_single(pipe)) }
}

impl<P, K, V> From<P> for XWidget<'static, PipeOptionWidget<K>>
where
  P: Pipe<Value = Option<V>>,
  V: Into<XWidget<'static, K>>,
  K: WidgetKind,
{
  fn from(pipe: P) -> Self { XWidget::new(pipe.build_single()) }
}

// ------ `Widget` to `XWidget` conversion -------

impl<'w> From<Widget<'w>> for XWidget<'w, Widget<'w>> {
  #[inline(always)]
  fn from(widget: Widget<'w>) -> Self { Self { widget, _kind: PhantomData } }
}

// ----- Into Widget --------------

impl<'w, W, K> IntoWidget<'w, K> for W
where
  W: Into<XWidget<'w, K>>,
  K: WidgetKind,
{
  fn into_widget(self) -> Widget<'w> { self.into().widget }
}
