use super::*;

/// The trait is used to enable child composition for `ComposeChild`.
pub trait ComposeWithChild<C, K: ?Sized> {
  type Target;
  fn with_child(self, child: C) -> Self::Target;
}

// ------ With child implementations ------
/// ComposeChild compose a type that can convert to its specific child type.
///
/// We choose to return a pair of parent and child instead of directly composing
/// and returning a `Widget`. This approach allows for continued composition
/// with certain child types like `Vec`.
pub struct NormalKind<K: ?Sized>(PhantomData<fn() -> K>);
impl<'c, P, C, K: ?Sized> ComposeWithChild<C, NormalKind<K>> for P
where
  P: ComposeChild<'c, Child: ChildFrom<C, K>>,
{
  type Target = Pair<State<P>, C>;

  #[inline]
  fn with_child(self, child: C) -> Self::Target { Pair { parent: State::value(self), child } }
}

struct TmlKind<K: ?Sized>(PhantomData<fn() -> K>);
impl<'c, P, C, Builder, K: ?Sized> ComposeWithChild<C, TmlKind<&'c K>> for P
where
  P: ComposeChild<'c, Child: Template<Builder = Builder>>,
  Builder: ChildFrom<C, K>,
{
  type Target = Pair<State<P>, Builder>;

  #[inline]
  fn with_child(self, child: C) -> Self::Target {
    Pair { parent: State::value(self), child: child.into_child() }
  }
}

impl<P, C, K: ?Sized> ComposeWithChild<C, dyn StateWriter<Value = K>> for P
where
  P: StateWriter<Value: ComposeWithChild<C, K>>,
{
  type Target = Pair<Self, C>;
  #[inline]
  fn with_child(self, child: C) -> Self::Target { Pair { parent: self, child } }
}

impl<P, C, K: ?Sized> ComposeWithChild<C, K> for FatObj<P>
where
  P: ComposeWithChild<C, K>,
{
  type Target = FatObj<P::Target>;

  #[track_caller]
  fn with_child(self, child: C) -> Self::Target {
    // Employing a verbose method to ensure accurate panic location reporting,
    // since the `closure_track_caller` macro is currently in an unstable state.
    // Once `closure_track_caller` becomes stable, a more concise alternative would
    // be: `self.map(|p| p.with_child(child))`
    let (host, fat) = self.into_parts();
    let child = host.with_child(child);
    fat.map(|_| child)
  }
}

impl<P, C, K: ?Sized> ComposeWithChild<C, K> for Pair<P, C>
where
  C: ComposeWithChild<C, K>,
{
  type Target = Pair<P, C::Target>;
  #[inline]
  fn with_child(self, child: C) -> Self::Target {
    let Pair { parent, child: c } = self;
    Pair { parent, child: c.with_child(child) }
  }
}

pub trait OptionComposeWithChild<'c, C, K: ?Sized> {
  fn with_child(self, child: C) -> Widget<'c>;
}
impl<'c, P, C, K: WidgetKind> OptionComposeWithChild<'c, C, K> for Option<P>
where
  P: ComposeWithChild<C, K>,
  C: IntoWidget<'c, K>,
  P::Target: IntoWidget<'c, K>,
{
  #[inline]
  fn with_child(self, child: C) -> Widget<'c> {
    if let Some(p) = self { p.with_child(child).into_widget() } else { child.into_widget() }
  }
}

// ---- convert to widget -------
impl<'w, P, C, K: ?Sized> From<Pair<P, C>> for XWidget<'w, OtherWidget<K>>
where
  P: StateWriter<Value: ComposeChild<'w, Child: ChildFrom<C, K>>>,
{
  #[inline]
  fn from(from: Pair<P, C>) -> Self {
    let Pair { parent, child } = from;
    let w = ComposeChild::compose_child(parent, child.into_child());
    XWidget::new(w)
  }
}

// ---------- Child Conversion ---------

// into kind
pub struct IntoKind;
impl<C, T> ChildFrom<C, IntoKind> for T
where
  C: Into<T>,
{
  #[inline]
  fn child_from(from: C) -> Self { from.into() }
}

// widget kind
impl<'a, W, K: ?Sized> ChildFrom<W, OtherWidget<K>> for Widget<'a>
where
  W: Into<XWidget<'a, OtherWidget<K>>>,
{
  fn child_from(child: W) -> Self { child.into_widget() }
}

impl<'a, W, K: ?Sized> ChildFrom<W, PipeOptionWidget<K>> for Widget<'a>
where
  W: Into<XWidget<'a, PipeOptionWidget<K>>>,
{
  fn child_from(child: W) -> Self { child.into_widget() }
}

// template builder from
pub struct TemplateBuilderKind<K: ?Sized>(PhantomData<fn() -> K>);
impl<Builder, C, K: ?Sized> ChildFrom<C, TemplateBuilderKind<K>> for Builder
where
  Builder: TemplateBuilder + ComposeWithChild<C, K, Target = Builder>,
{
  fn child_from(from: C) -> Self { Builder::default().with_child(from) }
}

// ---------- IntoChild implementation ----------------
impl<'a, C, T, K: ?Sized> IntoChild<C, K> for T
where
  C: ChildFrom<T, K>,
{
  fn into_child(self) -> C { C::child_from(self) }
}

/*

// impl Option as Template
impl<T> Template for Option<T> {
  type Builder = OptionBuilder<T>;

  #[inline]
  fn builder() -> Self::Builder { OptionBuilder(None) }
}

/// The template builder for `Option` introduces a new type to disambiguate the
/// `with_child` method call for `Option`, especially when `Option` acts as a
/// parent for a widget with `with_child` method.
pub struct OptionBuilder<T>(Option<T>);

impl<T> TemplateBuilder for OptionBuilder<T> {
  type Target = Option<T>;
  #[inline]
  fn build_tml(self) -> Self::Target { self.0 }
}

impl<T> ComposeChildFrom<OptionBuilder<T>, 1> for Option<T> {
  #[inline]
  fn compose_child_from(from: OptionBuilder<T>) -> Self { from.build_tml() }
}

impl<'w, C, T, const M: usize> ComposeWithChild<'w, C, false, 1, 0, M> for OptionBuilder<T>
where
  C: IntoChildCompose<T, M>,
{
  type Target = Self;

  #[inline]
  fn with_child(self, child: C) -> Self::Target { self.with_child(Some(child)) }
}

impl<'w, C, T, const M: usize> ComposeWithChild<'w, Option<C>, false, 1, 1, M> for OptionBuilder<T>
where
  C: IntoChildCompose<T, M>,
{
  type Target = Self;

  #[inline]
  fn with_child(mut self, child: Option<C>) -> Self::Target {
    debug_assert!(self.0.is_none(), "Option already has a child");
    self.0 = child.map(IntoChildCompose::into_child_compose);
    self
  }
}

// impl Vec<T> as Template

pub struct VecBuilder<T>(Vec<T>);

impl<T> Template for Vec<T> {
  type Builder = VecBuilder<T>;
  #[inline]
  fn builder() -> Self::Builder { VecBuilder(vec![]) }
}

impl<T> TemplateBuilder for VecBuilder<T> {
  type Target = Vec<T>;
  #[inline]
  fn build_tml(self) -> Self::Target { self.0 }
}

impl<T> ComposeChildFrom<VecBuilder<T>, 1> for Vec<T> {
  #[inline]
  fn compose_child_from(from: VecBuilder<T>) -> Self { from.build_tml() }
}

impl<'w, C, T, const M: usize> ComposeWithChild<'w, C, false, 1, 0, M> for VecBuilder<T>
where
  C: IntoChildCompose<T, M>,
{
  type Target = Self;

  #[inline]
  fn with_child(mut self, child: C) -> Self::Target {
    self.0.push(child.into_child_compose());
    self
  }
}

impl<'w, C, T, const N: usize, const M: usize> ComposeWithChild<'w, C, false, 2, N, M>
  for VecBuilder<T>
where
  T: Template,
  T::Builder: ComposeWithChild<'w, C, false, 1, N, M>,
  <T::Builder as ComposeWithChild<'w, C, false, 1, N, M>>::Target: TemplateBuilder<Target = T>,
{
  type Target = Self;

  #[inline]
  fn with_child(mut self, child: C) -> Self::Target {
    self
      .0
      .push(T::builder().with_child(child).build_tml());
    self
  }
}

impl<'w, C, T, const M: usize> ComposeWithChild<'w, C, false, 1, 1, M> for VecBuilder<T>
where
  C: IntoIterator,
  C::Item: IntoChildCompose<T, M>,
{
  type Target = Self;

  #[inline]
  fn with_child(mut self, child: C) -> Self::Target {
    self
      .0
      .extend(child.into_iter().map(|v| v.into_child_compose()));
    self
  }
}

impl ChildOfCompose for Resource<PixelImage> {}

 */

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_helper::{MockBox, MockStack};

  #[derive(Template)]
  enum PTml {
    Void(Void),
  }

  impl ChildOfCompose for Void {}

  struct P;

  impl ComposeChild<'static> for P {
    type Child = PTml;
    fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> Widget<'static> {
      Void.into_widget()
    }
  }

  #[derive(Declare)]
  struct XX;

  impl<'c> ComposeChild<'c> for XX {
    type Child = Widget<'c>;

    fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> Widget<'c> {
      Void.into_widget()
    }
  }

  #[test]
  fn template_fill_template() { let _ = |_: &BuildCtx| P.with_child(Void).into_widget(); }

  #[test]
  fn pair_compose_child() {
    let _ = |_: &BuildCtx| -> Widget {
      MockBox { size: ZERO_SIZE }
        .with_child(XX.with_child(Void {}))
        .into_widget()
    };
  }

  #[derive(Declare)]
  struct PipeParent;

  impl ComposeChild<'static> for PipeParent {
    type Child = BoxPipe<usize>;

    fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> Widget<'static> {
      Void.into_widget()
    }
  }

  #[test]
  fn compose_pipe_child() {
    let _value_child = fn_widget! {
      @PipeParent {  @ { BoxPipe::value(0) } }
    };

    let _pipe_child = fn_widget! {
      let state = State::value(0);
      @PipeParent {  @ { pipe!(*$state) } }
    };
  }

  #[test]
  fn compose_template_enum() {
    #[allow(dead_code)]
    #[derive(Template)]
    enum EnumTml {
      Widget(Widget<'static>),
      Text(TextInit),
    }

    #[derive(Declare)]
    struct EnumTest {}

    impl ComposeChild<'static> for EnumTest {
      type Child = Vec<EnumTml>;

      fn compose_child(_: impl StateWriter<Value = Self>, _: Self::Child) -> Widget<'static> {
        todo!()
      }
    }

    let _ = fn_widget! {
      let v = Stateful::new(true);
      let w = EnumTml::Widget(fn_widget! { @Void {} }.into_widget());
      @EnumTest {
        @ Void {}
        @ { "test" }
        @ { pipe!(*$v).map(|_| fn_widget! { @Void {} }) }
        @ MockStack { @Void {} }
        @ {w}
      }
    };
  }

  pub struct BuilderX;

  struct BuilderAKind<K: ?Sized>(PhantomData<fn() -> K>);
  struct BuilderBKind<K: ?Sized>(PhantomData<fn() -> K>);

  impl<C, K: ?Sized> ChildFrom<C, BuilderAKind<K>> for BuilderX
  where
    C: IntoChild<Widget<'static>, K>,
  {
    fn child_from(from: C) -> Self { todo!() }
  }

  impl<C, K: ?Sized> ChildFrom<C, BuilderBKind<K>> for BuilderX
  where
    C: IntoChild<CowArc<str>, K>,
  {
    fn child_from(from: C) -> Self { todo!() }
  }

  impl<'w, K: ?Sized, C> ComposeWithChild<C, BuilderAKind<K>> for BuilderX
  where
    C: IntoChild<Widget<'w>, K>,
  {
    type Target = Self;
    fn with_child(self, child: C) -> Self { todo!() }
  }

  impl<'w, K: ?Sized, C> ComposeWithChild<C, BuilderBKind<K>> for BuilderX
  where
    C: IntoChild<CowArc<str>, K>,
  {
    type Target = Self;
    fn with_child(self, child: C) -> Self { todo!() }
  }

  fn test_with_child() {
    let builder = BuilderX;
    let builder = builder.with_child("Hello");
    let builder = builder.with_child(Void);
    let builder: BuilderX = "hello".into_child();
    let builder: BuilderX = Void.into_child();
  }
}
