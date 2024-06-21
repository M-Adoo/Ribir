use super::*;
use crate::pipe::InnerPipe;

/// Trait specify what child a widget can have, and the target type is the
/// result of widget compose its child.
pub trait SingleWithChild<C, M: ?Sized> {
  // fixme: remove the ctx parameter
  fn with_child<'l>(self, child: C, ctx: &BuildCtx) -> Widget<'l>;
}

crate::widget::multi_build_replace_impl_include_self! {
  impl<P: SingleParent, C: {#}> SingleWithChild<C, dyn {#}> for P {
    #[track_caller]
    fn with_child<'l>(self, child: C, _: &BuildCtx) -> Widget<'l> {
      self.compose_child(child.into_widget())
    }
  }

  impl<P, C> SingleWithChild<Option<C>, dyn {#}> for P
  where
    P: SingleParent + RenderBuilder,
    C: {#},
  {
    #[track_caller]
    fn with_child<'l>(self, child: Option<C>, ctx: &BuildCtx) -> Widget<'l> {
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
    PP: InnerPipe<Value=Option<V>>,
    V: {#},
  {
    #[track_caller]
    fn with_child<'l>(self, child: PP, ctx: &BuildCtx) -> Widget<'l> {
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
