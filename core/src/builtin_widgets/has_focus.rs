use crate::prelude::*;
#[derive(PartialEq, Clone, Default)]
pub struct HasFocus {
  focused: bool,
}

impl HasFocus {
  pub fn has_focus(&self) -> bool { self.focused }
}

impl Declare for HasFocus {
  type Builder = FatObj<()>;
  #[inline]
  fn declarer() -> Self::Builder { FatObj::new(()) }
}

impl ComposeChild for HasFocus {
  type Child<'a> = Widget<'a>;
  fn compose_child<'a>(
    this: impl StateWriter<Value = Self> + 'a, child: Self::Child<'a>,
  ) -> impl FnWidget + 'a {
    fn_widget! {
      @ $child {
        on_focus_in: move|_| $this.write().focused = true,
        on_focus_out: move |_| $this.write().focused = false,
      }
    }
  }
}
