use super::*;
use crate::pipe::InnerPipe;

/// Trait specify what child a multi child widget can have, and the target type
/// after widget compose its child.
pub trait MultiWithChild<'l, C: 'l, M: ?Sized> {
  type P;
  fn with_child(self, child: C, ctx: &BuildCtx) -> MultiPair<'l, Self::P>;
}

pub struct MultiPair<'a, P> {
  pub parent: P,
  pub children: SmallVec<[Widget<'a>; 1]>,
}

trait FillVec<M: ?Sized> {
  // fixme: remove ctx parameter
  fn fill_vec(self, vec: &mut SmallVec<[Widget; 1]>, ctx: &BuildCtx);
}

crate::widget::multi_build_replace_impl_include_self! {
  impl<W: {#} + 'static> FillVec<dyn {#}> for W {
    #[inline]
    fn fill_vec(self, vec: &mut SmallVec<[Widget; 1]>, _: &BuildCtx) {
      vec.push(self.into_widget())
    }
  }

  impl<W> FillVec<&dyn {#}> for W
  where
    W: IntoIterator,
    W::Item: {#} + 'static,
  {
    #[inline]
    fn fill_vec(self, vec: &mut  SmallVec<[Widget; 1]>, _: &BuildCtx) {
      vec.extend(self.into_iter().map(|w| w.into_widget()))
    }
  }
}

crate::widget::multi_build_replace_impl_include_self! {
  impl<T, V> FillVec<&&dyn {#}> for T
  where
    T: InnerPipe<Value=V>,
    V: IntoIterator + 'static,
    V::Item: {#},
  {
    fn fill_vec(self, vec: &mut  SmallVec<[Widget; 1]>, ctx: &BuildCtx) {
      self.build_multi(vec, |v,ctx| v.build(ctx), ctx);
    }
  }
}

impl<'l, M: ?Sized, P, C: 'l> MultiWithChild<'l, C, M> for P
where
  P: MultiParent,
  C: FillVec<M>,
{
  type P = P;
  #[track_caller]
  fn with_child(self, child: C, ctx: &BuildCtx) -> MultiPair<'l, Self::P> {
    let mut children = SmallVec::default();
    child.fill_vec(&mut children, ctx);
    MultiPair { parent: self, children }
  }
}

impl<'l, M: ?Sized, C: 'l, P> MultiWithChild<'l, C, M> for MultiPair<'l, P>
where
  C: FillVec<M>,
{
  type P = P;
  #[inline]
  #[track_caller]
  fn with_child(mut self, child: C, ctx: &BuildCtx) -> MultiPair<'l, Self::P> {
    child.fill_vec(&mut self.children, ctx);
    self
  }
}

impl<'l, P: MultiParent> FnWidget for MultiPair<'l, P> {
  fn build(self, ctx: &BuildCtx) -> WidgetId {
    let MultiPair { parent, children } = self;
    parent.compose_children(children).build(ctx)
  }
}
