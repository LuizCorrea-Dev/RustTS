use logos::Logos;
use serde::{Deserialize, Serialize};

/// Rastreamento exato de posição no código-fonte `.rts` para diagnósticos determinísticos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Span {
    pub line_start: usize,
    pub col_start: usize,
    pub line_end: usize,
    pub col_end: usize,
    #[serde(skip)]
    pub byte_start: usize,
    #[serde(skip)]
    pub byte_end: usize,
}

impl Span {
    pub fn new(
        line_start: usize,
        col_start: usize,
        line_end: usize,
        col_end: usize,
        byte_start: usize,
        byte_end: usize,
    ) -> Self {
        Self {
            line_start,
            col_start,
            line_end,
            col_end,
            byte_start,
            byte_end,
        }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            line_start: self.line_start.min(other.line_start),
            col_start: if self.line_start <= other.line_start {
                self.col_start
            } else {
                other.col_start
            },
            line_end: self.line_end.max(other.line_end),
            col_end: if self.line_end >= other.line_end {
                self.col_end
            } else {
                other.col_end
            },
            byte_start: self.byte_start.min(other.byte_start),
            byte_end: self.byte_end.max(other.byte_end),
        }
    }
}

/// Conjunto Canônico de Tokens da linguagem RustTS (`.rts`).
/// Segue estritamente a Regra da Forma Canônica Única (Zero-Novelty Rust + TypeScript).
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\f]+")]
#[logos(skip r"//[^\n]*")]
pub enum TokenKind {
    #[token("\n")]
    Newline,

    // Palavras-chave de Estrutura e Módulo
    #[token("module")]
    KwModule,
    #[token("pub")]
    KwPub,
    #[token("fn")]
    KwFn,
    #[token("struct")]
    KwStruct,
    #[token("enum")]
    KwEnum,
    #[token("let")]
    KwLet,
    #[token("mut")]
    KwMut,
    #[token("return")]
    KwReturn,
    #[token("if")]
    KwIf,
    #[token("else")]
    KwElse,
    #[token("for")]
    KwFor,
    #[token("while")]
    KwWhile,
    #[token("break")]
    KwBreak,
    #[token("continue")]
    KwContinue,
    #[token("in")]
    KwIn,
    #[token("match")]
    KwMatch,
    #[token("import")]
    KwImport,
    #[token("extern")]
    KwExtern,
    #[token("impl")]
    KwImpl,
    #[token("as")]
    KwAs,

    // Modificador Nuclear de Capacidade (Zero Ambient Authority)
    #[token("cap")]
    KwCap,

    // Construtores Algébricos Canônicos
    #[token("Ok")]
    KwOk,
    #[token("Err")]
    KwErr,
    #[token("Some")]
    KwSome,
    #[token("None")]
    KwNone,

    // Tipos Primitivos Escalares + Família Completa de Aliases (TypeScript / C / C# / Unreal)
    #[token("i8")]
    #[token("int8")]
    #[token("sbyte")]
    TyI8,
    #[token("i16")]
    #[token("int16")]
    #[token("short")]
    TyI16,
    #[token("i32")]
    #[token("int32")]
    #[token("int")]
    TyI32,
    #[token("i64")]
    #[token("int64")]
    #[token("long")]
    TyI64,
    #[token("u8")]
    #[token("uint8")]
    #[token("byte")]
    TyU8,
    #[token("u16")]
    #[token("uint16")]
    #[token("ushort")]
    TyU16,
    #[token("u32")]
    #[token("uint32")]
    #[token("uint")]
    TyU32,
    #[token("u64")]
    #[token("uint64")]
    #[token("ulong")]
    TyU64,
    #[token("usize")]
    #[token("size_t")]
    #[token("uintptr")]
    TyUsize,
    #[token("f32")]
    #[token("float32")]
    #[token("float")]
    #[token("number")]
    TyF32,
    #[token("f64")]
    #[token("float64")]
    #[token("double")]
    TyF64,
    #[token("fix32")]
    #[token("fixed32")]
    TyFix32,
    #[token("bool")]
    #[token("boolean")]
    TyBool,
    #[token("str")]
    #[token("string")]
    TyStr,

    // Tipos Matemáticos Nativos SIMD-Ready
    #[token("vec2")]
    TyVec2,
    #[token("vec3")]
    TyVec3,
    #[token("vec4")]
    TyVec4,
    #[token("quat")]
    TyQuat,
    #[token("mat3")]
    TyMat3,
    #[token("mat4")]
    TyMat4,

    // Tipos Genéricos Fundamentais da Linguagem
    #[token("Option")]
    TyOption,
    #[token("Result")]
    TyResult,
    #[token("Handle")]
    TyHandle,

    // Atributos Declarativos Fixos do Compilador
    #[token("@component")]
    AttrComponent,
    #[token("@system")]
    AttrSystem,
    #[token("@parallel_system")]
    AttrParallelSystem,
    #[token("@unreal_actor")]
    AttrUnrealActor,
    #[token("@unreal_impl")]
    AttrUnrealImpl,
    #[token("@uproperty")]
    AttrUProperty,
    #[token("@gpu_kernel")]
    AttrGpuKernel,
    #[token("@shader_shared")]
    AttrShaderShared,
    #[token("@route")]
    AttrRoute,
    #[token("@sealed_sig")]
    AttrSealedSig,
    #[token("@open_sig")]
    AttrOpenSig,

    // Literais Booleanos
    #[token("true", |_| true)]
    #[token("false", |_| false)]
    BoolLit(bool),

    // Literais Numéricos Hexadecimais (ex: 0x48, 0xFFFF)
    #[regex(r"0x[0-9a-fA-F_]+", |lex| u64::from_str_radix(&lex.slice()[2..].replace('_', ""), 16).ok())]
    HexLit(u64),

    // Literais Ponto Flutuante
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*", |lex| lex.slice().replace('_', "").parse::<f64>().ok())]
    FloatLit(f64),

    // Literais Inteiros Decimais
    #[regex(r"[0-9][0-9_]*", |lex| lex.slice().replace('_', "").parse::<u64>().ok())]
    IntLit(u64),

    // Literais de String Imutável UTF-8
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        s[1..s.len() - 1].to_string()
    })]
    StringLit(String),

    // Identificadores
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    // Delimitadores e Pontuação
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("<")]
    LAngle,
    #[token(">")]
    RAngle,
    #[token(":")]
    Colon,
    #[token("::")]
    ColonColon,
    #[token(";")]
    Semicolon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("..")]
    DotDot,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("?")]
    Question,
    #[token("|")]
    Pipe,

    // Operadores Aritméticos, Relacionais e Lógicos
    #[token("=")]
    Assign,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("!")]
    Bang,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub lexeme: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

/// Executa a análise léxica de um arquivo `.rts` convertendo bytes em `Token`s com `Span` de linha e coluna.
/// Remove automaticamente o UTF-8 BOM (`\u{FEFF}`) caso o arquivo tenha sido salvo por editores Windows.
pub fn tokenize(raw_source: &str) -> Result<Vec<Token>, Vec<LexError>> {
    let source = raw_source.strip_prefix('\u{feff}').unwrap_or(raw_source);
    let mut lexer = TokenKind::lexer(source);
    let mut tokens = Vec::new();
    let mut errors = Vec::new();

    // Pré-computa offsets de início de cada linha para cálculo O(log N) de linha/coluna
    let mut line_starts = vec![0usize];
    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            line_starts.push(idx + 1);
        }
    }

    let offset_to_line_col = |offset: usize| -> (usize, usize) {
        let line_idx = match line_starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(insert_pos) => insert_pos.saturating_sub(1),
        };
        let line_number = line_idx + 1;
        let col_number = offset.saturating_sub(line_starts[line_idx]) + 1;
        (line_number, col_number)
    };

    while let Some(result) = lexer.next() {
        let range = lexer.span();
        let (line_start, col_start) = offset_to_line_col(range.start);
        let (line_end, col_end) = offset_to_line_col(range.end);
        let span = Span::new(
            line_start,
            col_start,
            line_end,
            col_end,
            range.start,
            range.end,
        );
        let lexeme = lexer.slice().to_string();

        match result {
            Ok(TokenKind::Newline) => {
                // Ignora quebras de linha na stream de tokens mantendo rastreamento preciso
            }
            Ok(kind) => tokens.push(Token {
                kind,
                span,
                lexeme,
            }),
            Err(()) => errors.push(LexError {
                message: format!("Invalid lexical token '{}' in RustTS source", lexeme),
                span,
            }),
        }
    }

    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}

