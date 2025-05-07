use crate::prelude::*;

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

// widget kind
impl<'a, W, K: ?Sized> RFrom<W, OtherWidget<K>> for Widget<'a>
where
  W: Into<XWidget<'a, OtherWidget<K>>>,
{
  fn r_from(child: W) -> Self { child.into_widget() }
}

impl<'a, W, K: ?Sized> RFrom<W, PipeOptionWidget<K>> for Widget<'a>
where
  W: Into<XWidget<'a, PipeOptionWidget<K>>>,
{
  fn r_from(child: W) -> Self { child.into_widget() }
}

// template builder from
pub struct TemplateBuilderKind<K: ?Sized>(PhantomData<fn() -> K>);
impl<Builder, C, K: ?Sized> RFrom<C, TemplateBuilderKind<K>> for Builder
where
  Builder: TemplateBuilder + ComposeWithChild<C, K, Target = Builder>,
{
  fn r_from(from: C) -> Self { Builder::default().with_child(from) }
}

// PairOf conversion
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

// ---------- IntoChild implementation ----------------
impl<'a, C, T, K: ?Sized> RInto<C, K> for T
where
  C: RFrom<T, K>,
{
  fn r_into(self) -> C { C::r_from(self) }
}
