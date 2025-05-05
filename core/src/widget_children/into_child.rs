use super::*;
use crate::{pipe::*, widget::*};

pub trait TemplateWithChild<C> {
  fn with_child<'c, K>(self, child: impl Into<OptionWidget<'c, K>>) -> Widget<'c>;
}

pub trait ChildFrom<C, K: ?Sized> {
  fn child_from(from: C) -> Self;
}

pub trait IntoChild<T, K: ?Sized> {
  fn into_child(self) -> T;
}

// ---------- Into Kind ----------------
/// This kind of child use `Into` trait to convert the child to `T`.

struct IntoKind;
impl<C, T> ChildFrom<C, IntoKind> for T
where
  C: Into<T>,
{
  #[inline]
  fn child_from(from: C) -> Self { from.into() }
}

// ---------- widget kind ----------------

impl<'a, W, K: ?Sized> ChildFrom<W, OtherWidget<K>> for Widget<'a>
where
  W: Into<XWidget<'a, OtherWidget<K>>>,
{
  fn child_from(child: W) -> Self { child.into_widget_x() }
}

impl<'a, W, K: ?Sized> ChildFrom<W, PipeOptionWidget<K>> for Widget<'a>
where
  W: Into<XWidget<'a, PipeOptionWidget<K>>>,
{
  fn child_from(child: W) -> Self { child.into_widget_x() }
}

// ---------- Template kind ----------------
impl<'a, B, T> ChildFrom<B, dyn TemplateBuilder<Target = T>> for T
where
  B: TemplateBuilder<Target = T>,
{
  fn child_from(from: B) -> Self { from.build_tml() }
}

// ---------- IntoChild implementation ----------------
impl<'a, C, T, K: ?Sized> IntoChild<C, K> for T
where
  C: ChildFrom<T, K>,
{
  fn into_child(self) -> C { C::child_from(self) }
}

// todo: remove --------- old convert impls ----------------
impl<T: ChildOfCompose> ComposeChildFrom<T, 0> for T {
  #[inline]
  fn compose_child_from(from: T) -> Self { from }
}

impl<F: FnMut() -> Widget<'static> + 'static> ComposeChildFrom<F, 1> for GenWidget {
  #[inline]
  fn compose_child_from(from: F) -> Self { GenWidget::new(from) }
}

impl<F: FnMut() -> W + 'static, W: IntoWidget<'static, M>, const M: usize>
  ComposeChildFrom<FnWidget<W, F>, M> for GenWidget
{
  #[inline]
  fn compose_child_from(from: FnWidget<W, F>) -> Self { GenWidget::from_fn_widget(from) }
}

impl<'w, F: FnOnce() -> W + 'w, W: IntoWidget<'w, M>, const M: usize> ComposeChildFrom<F, M>
  for FnWidget<W, F>
{
  #[inline]
  fn compose_child_from(from: F) -> Self { FnWidget::new(from) }
}

impl<'a, const M: usize, T: IntoWidget<'a, M>> ComposeChildFrom<T, M> for Widget<'a> {
  #[inline(always)]
  fn compose_child_from(from: T) -> Widget<'a> { from.into_widget() }
}

impl<W, C: ComposeChildFrom<T, M>, T, const M: usize> ComposeChildFrom<Pair<W, T>, M>
  for Pair<W, C>
{
  fn compose_child_from(from: Pair<W, T>) -> Pair<W, C> {
    let Pair { parent, child } = from;
    Pair { parent, child: C::compose_child_from(child) }
  }
}

impl<P: Pipe> ComposeChildFrom<P, 1> for BoxPipe<P::Value> {
  #[inline]
  fn compose_child_from(from: P) -> Self { BoxPipe::pipe(Box::new(from)) }
}

impl<U, const M: usize, T: DeclareInto<U, M>> ComposeChildFrom<T, M> for DeclareInit<U> {
  #[inline]
  fn compose_child_from(from: T) -> Self { from.declare_into() }
}

impl<T, C, const M: usize> IntoChildCompose<C, M> for T
where
  C: ComposeChildFrom<T, M>,
{
  fn into_child_compose(self) -> C { C::compose_child_from(self) }
}

impl<U: Into<CowArc<str>>> ComposeChildFrom<U, 1> for CowArc<str> {
  #[inline]
  fn compose_child_from(from: U) -> Self { from.into() }
}

// todo: remove---------- test impls ----------------

// /// A struct that holds a child and keep its kind, so can know how
// /// to convert it to `T` in the future.
// ///
// /// This type help us to process the child conversion in a generic way.
// pub struct XChild<T, F> {
//   child: T,
//   _from: PhantomData<F>,
// }

// impl<T, F> XChild<T, F> {
//   #[inline]
//   pub fn new(child: T) -> Self { Self { child, _from: PhantomData } }
// }

// impl From<i32> for A {
//   fn from(child: i32) -> Self { A }
// }

// impl From<bool> for B {
//   fn from(child: bool) -> Self { B }
// }

// struct A;
// struct B;

// enum ETml {
//   A(A),
//   B(B),
// }

// struct AKindOfETml<K>(PhantomData<K>);
// struct BKindOfETml<K>(PhantomData<K>);

// impl<C, K> From<C> for XChild<ETml, AKindOfETml<K>>
// where
//   C: Into<XChild<A, K>>,
// {
//   fn from(child: C) -> Self { XChild::new(ETml::A(child.into().child)) }
// }

// impl<C, K> From<C> for XChild<ETml, BKindOfETml<K>>
// where
//   C: Into<XChild<B, K>>,
// {
//   fn from(child: C) -> Self { XChild::new(ETml::B(child.into().child)) }
// }

// fn x() {
//   // IntoKind
//   let _w: XChild<CowArc<str>, _> = "Hello".into();

//   // Widget into XChild
//   let w: XChild<Widget<'_>, _> = Void.into();
//   // Widget self into XChild
//   let w: XChild<Widget<'_>, _> = Void.into_widget().into();

//   let e: XChild<B, _> = true.into();
//   let x = XWidget::from(Void);
//   let x = Void.into_widget_x();

//   let e: XChild<ETml, _> = 1.into();
//   let e: XChild<ETml, _> = true.into();
// }
