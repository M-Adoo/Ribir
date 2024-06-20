use super::*;
use crate::pipe::InnerPipe;

/// Trait specify what child a multi child widget can have, and the target type
/// after widget compose its child.
pub trait MultiWithChild<C, M: ?Sized> {
  type Target;
  fn with_child(self, child: C, ctx: &BuildCtx) -> Self::Target;
}

pub struct MultiPair<P> {
  pub parent: P,
  pub children: SmallVec<[Widget; 1]>,
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

impl<M: ?Sized, P, C> MultiWithChild<C, M> for P
where
  P: MultiParent,
  C: FillVec<M>,
{
  type Target = MultiPair<P>;
  #[track_caller]
  fn with_child(self, child: C, ctx: &BuildCtx) -> Self::Target {
    let mut children = SmallVec::default();
    child.fill_vec(&mut children, ctx);
    MultiPair { parent: self, children }
  }
}

impl<M: ?Sized, C, P> MultiWithChild<C, M> for MultiPair<P>
where
  C: FillVec<M>,
{
  type Target = Self;
  #[inline]
  #[track_caller]
  fn with_child(mut self, child: C, ctx: &BuildCtx) -> Self::Target {
    child.fill_vec(&mut self.children, ctx);
    self
  }
}

impl<P: MultiParent> FnWidget for MultiPair<P> {
  fn build(self, ctx: &BuildCtx) -> WidgetId {
    let MultiPair { parent, children } = self;
    parent.compose_children(children).build(ctx)
  }
}
