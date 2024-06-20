use super::*;
use crate::pipe::InnerPipe;

/// Trait specify what child a widget can have, and the target type is the
/// result of widget compose its child.
pub trait SingleWithChild<C, M: ?Sized> {
  type Target;
  // fixme: remove the ctx parameter
  fn with_child(self, child: C, ctx: &BuildCtx) -> Self::Target;
}

crate::widget::multi_build_replace_impl_include_self! {
  impl<P: SingleParent, C: {#} + 'static> SingleWithChild<C, dyn {#}> for P {
    type Target = Widget;
    #[track_caller]
    fn with_child(self, child: C, _: &BuildCtx) -> Self::Target {
      self.compose_child(child.into_widget())
    }
  }

  impl<P, C> SingleWithChild<Option<C>, dyn {#}> for P
  where
    P: SingleParent + RenderBuilder + 'static,
    C: {#} + 'static,
  {
    type Target = Widget;
    #[track_caller]
    fn with_child(self, child: Option<C>, ctx: &BuildCtx) -> Self::Target {
      if let Some(child) = child {
        self.with_child(child, ctx)
      } else {
        self.into_widget()
      }
    }
  }
}

crate::widget::multi_build_replace_impl_include_self! {
  impl<P, V, PP> SingleWithChild<PP, &dyn {#}> for P
  where
    P: SingleParent,
    PP: InnerPipe<Value=Option<V>> + 'static,
    V: {#} + 'static,
  {
    type Target = Widget;
    #[track_caller]
    fn with_child(self, child: PP, ctx: &BuildCtx) -> Self::Target {
      let child = child
        .map(|w| w.map_or_else(|| Void.into_widget(), |w| w.into_widget()))
        .into_widget();
      self.with_child(child, ctx)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_helper::MockBox;

  #[test]
  fn pair_with_child() {
    let mock_box = MockBox { size: ZERO_SIZE };
    let _ = |ctx| -> Widget {
      mock_box
        .clone()
        .with_child(mock_box.clone().with_child(mock_box, ctx), ctx)
        .build(ctx)
    };
  }

  #[test]
  fn fix_mock_box_compose_pipe_option_widget() {
    fn _x(w: BoxPipe<Option<Widget>>, ctx: &BuildCtx) {
      MockBox { size: ZERO_SIZE }.with_child(w.into_pipe(), ctx);
    }
  }
}
