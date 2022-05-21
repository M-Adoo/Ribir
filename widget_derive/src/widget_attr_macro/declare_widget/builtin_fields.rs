use ::builtin::builtin;
use inflector::Inflector;
use lazy_static::lazy_static;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use smallvec::{smallvec, SmallVec};
use std::collections::{BTreeMap, HashMap};
use syn::{parse_quote_spanned, spanned::Spanned};

use crate::{
  error::DeclareError,
  widget_attr_macro::{
    field_guard_variable, ribir_suffix_variable, widget_def_variable, widget_macro::EXPR_WIDGET,
    DeclareCtx, FollowPart, Follows,
  },
};

use super::{widget_gen::WidgetGen, DeclareField, DeclareWidget};

include!("../../builtin_fields_list.rs");

lazy_static! {
  pub static ref RESERVE_IDENT: HashMap<&'static str, &'static str, ahash::RandomState> = WIDGETS
    .iter()
    .flat_map(|w| w.fields.iter())
    .map(|f| (f.name, f.doc))
    .collect();
  pub static ref FIELD_WIDGET_TYPE: HashMap<&'static str, &'static str, ahash::RandomState> =
    WIDGETS
      .iter()
      .flat_map(|w| w.fields.iter().map(|f| (f.name, w.ty)))
      .collect();
  static ref BUILTIN_WIDGET_SUFFIX: HashMap<&'static str, String, ahash::RandomState> = WIDGETS
    .iter()
    .map(|w| (w.ty, w.ty.to_snake_case()))
    .collect();
}

#[derive(Debug, Default)]
pub struct BuiltinFieldWidgets {
  widgets: Vec<BuiltinWidgetInfo>,
}

#[derive(Debug)]
struct BuiltinWidgetInfo {
  name: &'static str,
  fields: SmallVec<[DeclareField; 1]>,
}

impl BuiltinFieldWidgets {
  pub fn as_builtin_widget(field_name: &Ident) -> Option<&String> {
    FIELD_WIDGET_TYPE
      .get(field_name.to_string().as_str())
      .and_then(|w| BUILTIN_WIDGET_SUFFIX.get(w))
  }

  pub fn visit_builtin_fields_mut(&mut self, ctx: &mut DeclareCtx) {
    self
      .widgets
      .iter_mut()
      .flat_map(|info| info.fields.iter_mut())
      .for_each(|f| ctx.visit_declare_field_mut(f))
  }

  pub fn all_builtin_fields(&self) -> impl Iterator<Item = &DeclareField> {
    self.widgets.iter().flat_map(|info| info.fields.iter())
  }

  pub fn collect_wrap_widget_follows<'a>(
    &'a self,
    host: &Ident,
    follows_info: &mut BTreeMap<Ident, Follows<'a>>,
  ) {
    self.widgets.iter().for_each(|info| {
      let follows: Follows = info
        .fields
        .iter()
        .filter_map(FollowPart::from_widget_field)
        .collect();

      if !follows.is_empty() {
        let name = ribir_suffix_variable(host, BUILTIN_WIDGET_SUFFIX.get(info.name).unwrap());
        follows_info.insert(name, follows);
      }
    });
  }

  pub fn key_follow_check(&self) -> crate::error::Result<()> {
    if let Some(info) = self.widgets.iter().find(|info| info.name == "Key") {
      assert_eq!(info.fields.len(), 1);
      let DeclareField { member, follows, .. } = &info.fields[0];
      if let Some(follows) = follows {
        return Err(DeclareError::KeyDependsOnOther {
          key: member.span().unwrap(),
          depends_on: follows.iter().map(|fo| fo.widget.span().unwrap()).collect(),
        });
      }
    }

    Ok(())
  }

  pub fn widget_tokens_iter<'a>(
    &'a self,
    host_id: Ident,
    ctx: &'a DeclareCtx,
  ) -> impl Iterator<Item = (Ident, TokenStream)> + 'a {
    self.widgets.iter().map(move |info| {
      let suffix = BUILTIN_WIDGET_SUFFIX.get(info.name).unwrap();
      let name = ribir_suffix_variable(&host_id, suffix);

      let span = info.span();
      let ty = Ident::new(&info.name, span).into();
      let fields = &info.fields;
      // If all fields have if guard and condition are false,  widget can
      // emit

      let def_and_ref_tokens = if let Some(conditions) = info.widget_guard_conditions() {
        let follows = fields.iter().filter_map(|f| f.follows.clone()).fold(
          None,
          |res: Option<Vec<_>>, follows| {
            if let Some(mut res) = res {
              res.extend(follows);
              Some(res)
            } else {
              Some(follows)
            }
          },
        );

        let gen = WidgetGen {
          ty: &ty,
          name: name.clone(),
          fields: &fields,
        };
        let widget_tokens = gen.gen_widget_tokens(ctx);
        if let Some(follows) = follows {
          let ty = Ident::new(EXPR_WIDGET, span).into();
          let wrap_name = widget_def_variable(&name);
          let mut expr_field: DeclareField = parse_quote_spanned! { span =>
            expr: { #conditions.then(|| { #widget_tokens #wrap_name}) }
          };
          expr_field.follows = Some(follows);
          let gen = WidgetGen {
            ty: &ty,
            name: name.clone(),
            fields: &[expr_field],
          };
          gen.gen_widget_tokens(ctx)
        } else {
          widget_tokens
        }
      } else {
        let gen = WidgetGen {
          ty: &ty,
          name: name.clone(),
          fields: &fields,
        };
        gen.gen_widget_tokens(ctx)
      };

      (name, def_and_ref_tokens)
    })
  }

  /// return builtin fields composed tokens, and the upstream tokens if the
  /// finally widget as a expr widget.
  pub fn compose_tokens(&self, host: &DeclareWidget) -> TokenStream {
    let host_name = host.widget_identify();
    let mut compose_tokens = quote! {};
    self
      .widgets
      .iter()
      .fold(host.is_host_expr_widget(), |is_expr_widget, info| {
        let suffix = BUILTIN_WIDGET_SUFFIX.get(info.name).unwrap();
        let name = ribir_suffix_variable(&host_name, suffix);
        let wrap_def = widget_def_variable(&name);
        let host_def = widget_def_variable(&host_name);
        let span = info.span();
        if is_expr_widget {
          compose_tokens.extend(
            quote_spanned! { span => let #host_def = SingleChildWidget::new(#wrap_def, #host_def.into_single_child()); },
          )
        } else {
          compose_tokens.extend(
            quote_spanned! { span =>  let #host_def = SingleChildWidget::new(#wrap_def, Some(#host_def.into_widget())); },
          )
        }
        info.is_expr_widget()
      });
    compose_tokens
  }

  pub fn assign_builtin_field(
    &mut self,
    widget_ty: &'static str,
    field: DeclareField,
  ) -> syn::Result<()> {
    assert_eq!(
      FIELD_WIDGET_TYPE.get(field.member.to_string().as_str()),
      Some(&widget_ty)
    );

    let fields: &mut SmallVec<_> =
      if let Some(info) = self.widgets.iter_mut().find(|info| info.name == widget_ty) {
        &mut info.fields
      } else {
        let info = BuiltinWidgetInfo { name: widget_ty, fields: smallvec![] };
        self.widgets.push(info);
        let w = self.widgets.last_mut();
        &mut w.unwrap().fields
      };
    if fields.iter().find(|f| f.member == field.member).is_some() {
      return Err(syn::Error::new(
        field.span(),
        format!("field `{}` specified more than once", stringify!($name)).as_str(),
      ));
    }
    fields.push(field);
    Ok(())
  }

  pub fn finally_is_expr_widget(&self) -> Option<bool> {
    self.widgets.last().map(BuiltinWidgetInfo::is_expr_widget)
  }
}



impl BuiltinWidgetInfo {
  fn is_expr_widget(&self) -> bool {
    self.fields.iter().all(|f| f.if_guard.is_some())
  }

  fn widget_guard_conditions(&self) -> Option<TokenStream> {
    self.is_expr_widget().then(|| {
      let conditions = self.fields
        .iter()
        .map(|f| field_guard_variable(&f.member, f.if_guard.span()));
      quote! {( #(#conditions) || * )}
    })
    
  }

  fn span(&self) -> Span {
    self
      .fields
      .iter()
      .fold(None, |span: Option<Span>, f| {
        if let Some(span) = span {
          span.join(f.member.span())
        } else {
          Some(f.member.span())
        }
      })
      .unwrap()
  }
}