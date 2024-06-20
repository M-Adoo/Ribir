#[doc(hidden)]
pub use std::{
  any::{Any, TypeId},
  marker::PhantomData,
  ops::Deref,
};

pub(crate) use crate::widget_tree::*;
use crate::{context::*, prelude::*, render_helper::PureRender};
pub trait Compose: Sized {
  /// Describes the part of the user interface represented by this widget.
  /// Called by framework, should never directly call it.
  fn compose(this: impl StateWriter<Value = Self>) -> impl FnWidget;
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
  fn perform_layout(&self, clamp: BoxClamp, ctx: &mut LayoutCtx) -> Size;

  /// `paint` is a low level trait to help you draw your widget to paint device
  /// across `PaintingCtx::painter` by itself coordinate system. Not care
  /// about children's paint in this method, framework will call children's
  /// paint individual. And framework guarantee always paint parent before
  /// children.
  fn paint(&self, ctx: &mut PaintingCtx);

  /// Whether the constraints from parent are the only input to detect the
  /// widget size, and child nodes' size not affect its size.
  fn only_sized_by_parent(&self) -> bool { false }

  /// Determines the set of render widgets located at the given position.
  fn hit_test(&self, ctx: &HitTestCtx, pos: Point) -> HitTest {
    let is_hit = hit_test_impl(ctx, pos);
    HitTest { hit: is_hit, can_hit_child: is_hit }
  }

  fn get_transform(&self) -> Option<Transform> { None }
}

/// The common type of all widget can convert to.
/// todo: 迭代创建, 可以组合孩子
pub struct Widget(Box<dyn for<'a, 'b> FnOnce(&'a BuildCtx<'b>) -> WidgetId>);

/// A boxed function widget that can be called multiple times to regenerate
/// widget.
pub struct GenWidget(Box<dyn for<'a, 'b> FnMut(&'a BuildCtx<'b>) -> Widget>);

/// Trait to build a indirect widget into widget tree with `BuildCtx` in the
/// build phase. You should not implement this trait directly, framework will
/// auto implement this.
///
/// A indirect widget is a widget that is not `Compose`, `Render` and
/// `ComposeChild`,  like function widget and  `Pipe<Widget>`.
pub trait FnWidget {
  /// Builds the widget using the provided `BuildCtx`.
  ///
  /// ## Notice
  ///
  /// In Ribir, the widget tree is always constructed from the root to the leaf.
  /// This approach ensures that descendants can access the context shared by
  /// their ancestors.
  ///
  /// Therefore, you should avoid directly calling the build method unless your
  /// widget does not require any context information.
  ///
  /// Additionally, if you invoke the `build` method but fail to append the
  /// returned ID to the parent, it may result in a memory leak.
  fn build(self, ctx: &BuildCtx) -> WidgetId;

  /// Converts a function-based widget into a standard widget type.
  ///
  /// # Example
  ///
  /// ```ignore
  /// let w: Widget = if xxx {
  ///   fn_widget! { ... }.into_widget()
  /// else {
  ///   fn_widget! { ... }.into_widget()
  /// };
  /// ```
  fn into_widget(self) -> Widget
  where
    Self: Sized + 'static,
  {
    Widget(Box::new(move |ctx| self.build(ctx)))
  }
}

/// Trait to build a compose widget into widget tree with `BuildCtx` in the
/// build phase. You should not implement this trait directly, implement
/// `Compose` trait instead.
pub trait ComposeBuilder {
  /// See [`FnWidget::build`].
  fn build(self, ctx: &BuildCtx) -> WidgetId;

  /// See [`FnWidget::into_widget`].
  fn into_widget(self) -> Widget
  where
    Self: Sized + 'static,
  {
    Widget(Box::new(move |ctx| self.build(ctx)))
  }
}

/// Trait to build a render widget into widget tree with `BuildCtx` in the build
/// phase. You should not implement this trait directly, implement `Render`
/// trait instead.
pub trait RenderBuilder {
  /// See [`FnWidget::build`].
  fn build(self, ctx: &BuildCtx) -> WidgetId;

  /// See [`FnWidget::into_widget`].
  fn into_widget(self) -> Widget
  where
    Self: Sized + 'static,
  {
    Widget(Box::new(move |ctx| self.build(ctx)))
  }
}

/// Trait to build a `ComposeChild` widget without child into widget tree with
/// `BuildCtx` in the build phase, only work if the child of `ComposeChild` is
/// `Option<>_`  . You should not implement this trait directly,
/// implement `ComposeChild` trait instead.
pub trait ComposeChildBuilder {
  /// See [`FnWidget::build`].
  fn build(self, ctx: &BuildCtx) -> WidgetId;

  /// See [`FnWidget::into_widget`].
  fn into_widget(self) -> Widget
  where
    Self: Sized + 'static,
  {
    Widget(Box::new(move |ctx| self.build(ctx)))
  }
}

/// Trait only for `Widget`, you should not implement this trait.
pub trait SelfBuilder {
  /// See [`FnWidget::build`].
  fn build(self, ctx: &BuildCtx) -> WidgetId;

  /// See [`FnWidget::into_widget`].
  fn into_widget(self) -> Widget;
}

impl SelfBuilder for Widget {
  #[inline(always)]
  fn build(self, ctx: &BuildCtx) -> WidgetId { (self.0)(ctx) }

  #[inline(always)]
  fn into_widget(self) -> Widget { self }
}

impl<F> FnWidget for F
where
  F: FnOnce(&BuildCtx) -> WidgetId,
{
  #[inline]
  fn build(self, ctx: &BuildCtx) -> WidgetId { self(ctx) }
}

impl FnWidget for GenWidget {
  #[inline]
  fn build(mut self, ctx: &BuildCtx) -> WidgetId { self.gen_widget(ctx).build(ctx) }
}

impl GenWidget {
  pub fn new(mut f: impl FnMut(&BuildCtx) -> WidgetId + 'static) -> Self {
    Self(Box::new(move |ctx| {
      let id = f(ctx);
      (move |_: &BuildCtx| id).into_widget()
    }))
  }

  #[inline]
  pub fn gen_widget(&mut self, ctx: &BuildCtx) -> Widget { (self.0)(ctx) }
}

impl<F: FnMut(&BuildCtx) -> WidgetId + 'static> From<F> for GenWidget {
  #[inline]
  fn from(f: F) -> Self { Self::new(f) }
}

impl<C: Compose + 'static> ComposeBuilder for C {
  #[inline]
  fn build(self, ctx: &BuildCtx) -> WidgetId { Compose::compose(State::value(self)).build(ctx) }
}

impl<R: Render + 'static> RenderBuilder for R {
  fn build(self, ctx: &BuildCtx) -> WidgetId { ctx.alloc_widget(Box::new(PureRender(self))) }
}

impl<W: ComposeChild<Child = Option<C>> + 'static, C> ComposeChildBuilder for W {
  #[inline]
  fn build(self, ctx: &BuildCtx) -> WidgetId {
    ComposeChild::compose_child(State::value(self), None).build(ctx)
  }
}

pub(crate) fn hit_test_impl(ctx: &HitTestCtx, pos: Point) -> bool {
  ctx
    .box_rect()
    .map_or(false, |rect| rect.contains(pos))
}

macro_rules! _replace {
  (@replace($n: path) [$($e:tt)*] {#} $($rest:tt)*) => {
    $crate::widget::_replace!(@replace($n) [$($e)* $n] $($rest)*);
  };
  (@replace($n: path) [$($e:tt)*] $first: tt $($rest:tt)*) => {
    $crate::widget::_replace!(@replace($n) [$($e)* $first] $($rest)*);
  };
  (@replace($i: path) [$($e:tt)*]) => { $($e)* };
  (@replace($n: path) $first: tt $($rest:tt)*) => {
    $crate::widget::_replace!(@replace($n) [$first] $($rest)*);
  };
}

macro_rules! multi_build_replace_impl {
  ($($rest:tt)*) => {
    $crate::widget::repeat_and_replace!([
      $crate::widget::ComposeBuilder,
      $crate::widget::RenderBuilder,
      $crate::widget::ComposeChildBuilder,
      $crate::widget::FnWidget
    ] $($rest)*);
  };
}

macro_rules! multi_build_replace_impl_include_self {
  ($($rest:tt)*) => {
    $crate::widget::multi_build_replace_impl!($($rest)*);
    $crate::widget::_replace!(@replace($crate::widget::SelfBuilder) $($rest)*);
  };
  ({} $($rest:tt)*) => {}
}

macro_rules! repeat_and_replace {
  ([$first: path $(,$n: path)*] $($rest:tt)*) => {
    $crate::widget::_replace!(@replace($first) $($rest)*);
    $crate::widget::repeat_and_replace!([$($n),*] $($rest)*);
  };
  ([] $($rest:tt)*) => {
  };
}

pub(crate) use _replace;
pub(crate) use multi_build_replace_impl;
pub(crate) use multi_build_replace_impl_include_self;
pub(crate) use repeat_and_replace;
