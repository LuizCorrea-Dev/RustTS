/// Exportador oficial da gramática GBNF (GGML BNF) da linguagem RustTS (`.rts`).
/// Projetado para Constrained Decoding em motores LLM (`llama.cpp`, `vLLM`, `Structured Outputs`),
/// tornando erros sintáticos matematicamente impossíveis (0% Syntax Hallucination Rate).
pub fn export_rustts_gbnf() -> &'static str {
    r#"# ==============================================================================
# RustTS (.rts) Official Formal Grammar for Constrained LLM Decoding (GBNF)
# Compiler: rtsc 1.4.0-rts | Specification: RustTS_guia.md (One Canonical Way)
# ==============================================================================

root            ::= ws module-decl? (import-decl)* (item ws)*

module-decl     ::= "module" ws1 module-path ws ";" ws
import-decl     ::= "import" ws1 module-path ws ";" ws
module-path     ::= ident ("." ident)*

item            ::= attribute-list (struct-decl | enum-decl | extern-fn-decl | impl-decl | fn-decl)
extern-fn-decl  ::= ("pub" ws1)? "extern" (ws1 string-lit)? ws1 "fn" ws1 ident ws "(" ws param-list? ws ")" (ws "->" ws type-expr)? ws ";"
impl-decl       ::= "impl" ws1 ident ws "{" ws (attribute-list fn-decl ws)* "}"

attribute-list  ::= (attribute ws)*
attribute       ::= "@component"
                  | "@gpu_kernel"
                  | "@shader_shared"
                  | "@sealed_sig"
                  | "@open_sig"
                  | "@system" ws "(" ws "stage" ws "=" ws string-lit ws ")"
                  | "@parallel_system" ws "(" ws "stage" ws "=" ws string-lit ws ")"
                  | "@unreal_actor" ws "(" ws "replicate" ws "=" ws bool-lit ws "," ws "category" ws "=" ws string-lit ws ")"
                  | "@unreal_impl" ws "(" ws ident ws ")"
                  | "@uproperty" ws "(" ws ident (ws "," ws ident)* ws ")"
                  | "@route" ws "(" ws string-lit ws "," ws string-lit ws ")"

struct-decl     ::= "struct" ws1 ident ws "{" ws (struct-field ws)* "}"
struct-field    ::= attribute-list ident ws ":" ws type-expr ws ","?

enum-decl       ::= "enum" ws1 ident ws "{" ws (enum-variant ws)* "}"
enum-variant    ::= ident (ws "(" ws type-expr ws ")")? ws ","?

fn-decl         ::= ("pub" ws1)? "fn" ws1 ident ws "(" ws param-list? ws ")" (ws "->" ws type-expr)? ws block
param-list      ::= param (ws "," ws param)* ws ","?
param           ::= ident ws ":" ws ("cap" ws1 capability-spec | type-expr)

capability-spec ::= cap-domain (ws "<" ws (ident | hex-lit | int-lit) ws ">")?
cap-domain      ::= "I2CBus" | "I2CRead" | "I2CPort" | "PWMPin" | "PWMWrite"
                  | "GPIOPin" | "SPIBus" | "DMAChannel" | "Interrupt"
                  | "MonotonicClock" | "TimeStep"
                  | "ReadColumn" | "MutColumn" | "Read" | "Mut"
                  | "StaticArena" | "LevelArena" | "SessionArena" | "FrameArena" | "RequestArena" | "TaskArena"
                  | "ReadFrameArena" | "ReadRequestArena" | "ReadTaskArena"
                  | "NetSocket" | "TcpSocket" | "UdpTick" | "HttpRoute"
                  | "SqlRead" | "SqlWrite"
                  | "GpuQueue" | "WebGpuDevice" | "CanvasContext" | "AudioBuffer"

type-expr       ::= primitive-type
                  | math-type
                  | "[" ws type-expr ws ";" ws int-lit ws "]"
                  | "Option" ws "<" ws type-expr ws ">"
                  | "Result" ws "<" ws type-expr ws "," ws type-expr ws ">"
                  | "Handle" ws "<" ws type-expr ws ">"
                  | ident (ws "<" ws type-expr (ws "," ws type-expr)* ws ">")?

primitive-type  ::= "i8" | "i16" | "i32" | "i64"
                  | "u8" | "u16" | "u32" | "u64" | "usize"
                  | "f32" | "f64" | "fix32" | "bool" | "str"

math-type       ::= "vec2" | "vec3" | "vec4" | "quat" | "mat3" | "mat4"

block           ::= "{" ws (stmt ws)* "}"

stmt            ::= let-stmt
                  | return-stmt
                  | if-stmt
                  | match-stmt
                  | for-stmt
                  | while-stmt
                  | "break" ws ";"
                  | "continue" ws ";"
                  | assign-or-expr-stmt

let-stmt        ::= "let" ws1 ("mut" ws1)? ident (ws ":" ws type-expr)? ws "=" ws expr ws ";"
return-stmt     ::= "return" (ws1 expr)? ws ";"
if-stmt         ::= "if" ws1 expr ws block (ws "else" ws (if-stmt | block))?
match-stmt      ::= "match" ws1 expr ws "{" ws (match-arm ws)* "}"
match-arm       ::= match-pattern ws "=>" ws (block | stmt) (ws ",")?
match-pattern   ::= "_"
                  | ("Ok" | "Err" | "Some") ws "(" ws ident ws ")"
                  | "None"
                  | ident ("::" ident)? (ws "(" ws ident ws ")")?
for-stmt        ::= "for" ws1 ident ws1 "in" ws1 expr ws ".." ws expr ws block
while-stmt      ::= "while" ws1 expr ws block
assign-or-expr-stmt ::= expr (ws "=" ws expr)? ws ";"

expr            ::= unary-expr (ws binary-op ws unary-expr)*
unary-expr      ::= (unary-op ws)* postfix-expr
unary-op        ::= "-" | "!"
postfix-expr    ::= primary-expr (postfix-op)*
postfix-op      ::= "." ident (ws "(" ws arg-list? ws ")")?
                  | "[" ws expr ws "]"
                  | "?"
                  | ws1 "as" ws1 type-expr

primary-expr    ::= "|" ws ident ws "|" ws expr
                  | float-lit
                  | hex-lit
                  | int-lit
                  | bool-lit
                  | string-lit
                  | ("Ok" | "Err" | "Some") ws "(" ws expr ws ")"
                  | "None"
                  | ident "::" ident (ws "(" ws arg-list? ws ")")?
                  | ident ws "{" ws field-init-list? ws "}"
                  | ident (ws "(" ws arg-list? ws ")")?
                  | "[" ws (expr (ws "," ws expr)*)? ws "]"
                  | "(" ws expr ws ")"

arg-list        ::= expr (ws "," ws expr)*
field-init-list ::= field-init (ws "," ws field-init)* ws ","?
field-init      ::= ident ws ":" ws expr

binary-op       ::= "+" | "-" | "*" | "/" | "==" | "!=" | "<=" | ">=" | "<" | ">" | "&&" | "||"

ident           ::= [a-zA-Z_] [a-zA-Z0-9_]*
hex-lit         ::= "0x" [0-9a-fA-F_]+
float-lit       ::= [0-9]+ "." [0-9]+
int-lit         ::= [0-9]+
bool-lit        ::= "true" | "false"
string-lit      ::= "\"" [^"\\]* "\""

ws              ::= ([ \t\r\n] | "//" [^\n]* "\n")*
ws1             ::= ([ \t\r\n] | "//" [^\n]* "\n")+
"#
}
