use ribir_core::prelude::*;
use std::time::Duration;
#[derive(Declare)]
pub struct CaretStyle {
  pub font: TextStyle,
}

impl ComposeStyle for CaretStyle {
  type Host = Widget;
  fn compose_style(this: Stateful<Self>, host: Self::Host) -> Widget
  where
    Self: Sized,
  {
    widget! {
      states { this }
      DynWidget {
        id: caret,
        opacity: 1.,
        background: this.font.foreground.clone(),
        mounted: move |_| animate1.run(),
        dyns: host,
      }
      Animate {
        id: animate1,
        prop: prop!(caret.opacity),
        from: 0.,
        transition: Transition {
          easing: easing::steps(2, easing::StepsJump::JumpNone),
          duration: Duration::from_secs(1),
          repeat: Some(f32::INFINITY),
          delay: None
        }
      }
    }
  }
}