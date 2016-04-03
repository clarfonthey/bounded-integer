use syntax::ast::{
    self,
    Attribute,
    EnumDef,
    Expr,
    Ident,
    Item,
    ItemKind,
    Mac_,
    TokenTree,
    Variant,
    Visibility,
};
use syntax::codemap::{self, Span};
use syntax::errors::DiagnosticBuilder;
use syntax::ext::base::ExtCtxt;
use syntax::ext::build::AstBuilder;
use syntax::parse::token::{DelimToken, InternedString, Token};
use syntax::parse::token::keywords::Keyword;
use syntax::parse::token::special_idents;
use syntax::ptr::P;

use IntLit;

/// Parsed bounded integer enum.
#[derive(Debug)]
pub struct IntegerEnum {
    /// Attributes.
    pub attrs: Vec<Attribute>,

    /// Visibility.
    pub is_pub: bool,

    /// Name.
    pub name: Ident,

    /// Representation.
    pub repr: Ident,

    /// Minimum value.
    pub min: P<Expr>,

    /// Maximum value.
    pub max: P<Expr>,
}

impl IntegerEnum {
    /// Parses a slice of token trees.
    ///
    /// Roughly equivalent to:
    ///
    /// ```text
    /// $(#[$attr:meta])*
    /// $(pub)? enum $name:ident: $repr:ident { $min:expr...$max:expr }
    /// ```
    pub fn parse_tts<'a>(
        cx: &'a ExtCtxt,
        tts: &[TokenTree],
    ) -> Result<Self, DiagnosticBuilder<'a>> {
        let mut parser = cx.new_parser_from_tts(tts);

        // $(#[$attr:meta])*
        let attrs = try!(parser.parse_outer_attributes());

        // $(pub)? enum
        let is_pub = parser.eat_keyword(Keyword::Pub);
        try!(parser.expect_keyword(Keyword::Enum));

        // $name:ident: $repr:ident
        let name = try!(parser.parse_ident());
        try!(parser.expect(&Token::Colon));
        let repr = try!(parser.parse_ident());

        // { $min:expr...$max:expr }
        try!(parser.expect(&Token::OpenDelim(DelimToken::Brace)));
        let min = try!(parser.parse_pat_literal_maybe_minus());
        try!(parser.expect(&Token::DotDotDot));
        let max = try!(parser.parse_pat_literal_maybe_minus());
        try!(parser.expect(&Token::CloseDelim(DelimToken::Brace)));

        try!(parser.expect(&Token::Eof));

        Ok(IntegerEnum {
            attrs: attrs,
            is_pub: is_pub,
            name: name,
            repr: repr,
            min: min,
            max: max,
        })
    }

    /// Creates an enum item and a `bounded_integer_impls` macro invocation item.
    ///
    /// - Adds `#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]`
    /// - Adds `#[repr($repr)]`
    /// - Generates variants of the form `...N1, Z0, P1...`.
    /// - Sets item visibility.
    pub fn into_items(mut self, cx: &ExtCtxt, sp: Span) -> Vec<P<Item>> {
        self.add_derives(cx, sp);
        self.add_repr(cx, sp);

        let variants = self.variants(cx);
        let impls_macro_item = self.impls_macro_item(&variants, cx, sp);

        let enum_def = EnumDef { variants: variants };
        let enum_kind = ItemKind::Enum(enum_def, Default::default());
        let is_pub = self.is_pub;
        let enum_item = cx.item(sp, self.name, self.attrs, enum_kind).map(|mut item| {
            if is_pub { item.vis = Visibility::Public; }
            item
        });

        vec![enum_item, impls_macro_item]
    }

    /// Adds `#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]` to the attributes.
    fn add_derives(&mut self, cx: &ExtCtxt, sp: Span) {
        let derives = ["Clone", "Copy", "PartialEq", "Eq", "PartialOrd", "Ord"].iter()
            .map(|s| InternedString::new(s))
            .map(|s| cx.meta_word(sp, s))
            .collect();
        let derive_list = cx.meta_list(sp, InternedString::new("derive"), derives);
        self.attrs.push(cx.attribute(sp, derive_list));
    }

    /// Adds `#[repr($repr)]` to the attributes.
    fn add_repr(&mut self, cx: &ExtCtxt, sp: Span) {
        let repr = cx.meta_word(sp, self.repr.name.as_str());
        let repr_list = cx.meta_list(sp, InternedString::new("repr"), vec![repr]);
        self.attrs.push(cx.attribute(sp, repr_list));
    }

    /// Generates variants for the range of the form `N1, Z0, P1`.
    fn variants(&self, cx: &ExtCtxt) -> Vec<Variant> {
        let mut vec = Vec::new();
        let mut current = self.min.clone();
        loop {
            let int_lit = IntLit::from_expr(&*current).unwrap(); // FIXME
            let mut variant = cx.variant(current.span, int_lit.into_ident(cx), vec![]);
            variant.node.disr_expr = Some(current);
            vec.push(variant);

            // FIXME: Infinite loop risk.
            if Ok(int_lit) == IntLit::from_expr(&*self.max) { break; }
            current = int_lit.succ().into_expr(cx, self.min.span);
        }
        vec
    }

    /// Creates a `bounded_integer_impls` macro invocation item.
    fn impls_macro_item(&self, variants: &[Variant], cx: &ExtCtxt, sp: Span) -> P<Item> {
        let path = cx.path_ident(sp, cx.ident_of("bounded_integer_impls"));
        let mac = codemap::respan(sp, Mac_ {
            path: path,
            tts: vec![],
            ctxt: ast::EMPTY_CTXT,
        });
        cx.item(sp, special_idents::invalid, vec![], ItemKind::Mac(mac))
    }
}
