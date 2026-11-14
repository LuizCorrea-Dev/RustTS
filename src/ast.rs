use crate::lexer::Span;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Unidade de Compilação Canônica de um arquivo `.rts`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    /// Caminho hierárquico do módulo, ex: `["drone", "thermal_control"]` ou `["engine", "physics"]`
    pub path: Vec<String>,
    /// Lista de caminhos de módulos importados explicitamente (`import a.b.c;`)
    #[serde(default)]
    pub imports: Vec<Vec<String>>,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Item {
    Struct(StructDecl),
    Enum(EnumDecl),
    Function(FnDecl),
    ExternFn(ExternFnDecl),
}

/// Atributos declarativos fixos do compilador (proíbe macros procedurais arbitrárias)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Attribute {
    /// `@component`: Marca uma struct para layout Structure of Arrays (SoA) alinhado em 64 bytes (SIMD)
    Component,
    /// `@system(stage = "FixedUpdate")`: Sistema ECS single-thread determinístico com cursor direto de Arena
    System { stage: String },
    /// `@parallel_system(stage = "FixedUpdate")`: Sistema ECS multi-core com Atomic Tile Batching na Arena
    ParallelSystem { stage: String },
    /// `@unreal_actor(replicate = true, category = "Combat")`: Gera AActor `.h`/`.cpp` compatível com UHT
    UnrealActor { replicate: bool, category: String },
    /// `@unreal_impl(WeaponProjectile)`: Associa métodos UFUNCTION a um Actor/Component da Unreal
    UnrealImpl { target_struct: String },
    /// `@uproperty(edit_anywhere, blueprint_read_only, replicated)`
    UProperty { flags: Vec<String> },
    /// `@gpu_kernel`: Unificação CPU-GPU (SPIR-V / DXIL / WGSL)
    GpuKernel,
    /// `@shader_shared`: Struct com alinhamento determinístico CPU/GPU
    ShaderShared,
    /// `@route("GET", "/api/v1/users/:id")`: Endpoint de servidor com `RequestArena`
    Route { method: String, path: String },
    /// `@sealed_sig`: Contrato de assinatura imutável (padrão de segurança: IA proibida de escalar privilégios)
    SealedSig,
    /// `@open_sig`: Permite que o diagnóstico sugira adição de `cap` na assinatura durante prototipação arquitetural
    OpenSig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructDecl {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub fields: Vec<StructField>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructField {
    pub attributes: Vec<Attribute>,
    pub name: String,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FnDecl {
    pub attributes: Vec<Attribute>,
    pub is_pub: bool,
    pub name: String,
    pub params: Vec<FnParam>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Declaração FFI C-ABI (`extern "C" fn nome(args..., cap_fantasma: cap Dominio) -> Ret;`)
/// Os parâmetros `cap` impõem verificação estática de autoridade sem alterar a ABI C real!
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternFnDecl {
    pub attributes: Vec<Attribute>,
    pub is_pub: bool,
    pub abi: String,
    pub name: String,
    pub params: Vec<FnParam>,
    pub return_type: Option<TypeExpr>,
    pub span: Span,
}

impl FnDecl {
    /// Por padrão de segurança (Zero Privilege Escalation), toda assinatura de função é uma fronteira
    /// de segurança selada (`@sealed_sig`), a menos que o arquiteto marque explicitamente `@open_sig`.
    pub fn is_sealed_contract(&self) -> bool {
        !self
            .attributes
            .iter()
            .any(|attr| matches!(attr, Attribute::OpenSig))
    }

    /// Renderiza a assinatura canônica da função (usada apenas quando `@open_sig` está ativo)
    pub fn format_signature(&self, extra_cap: Option<(&str, &CapabilitySpec)>) -> String {
        let vis = if self.is_pub { "pub " } else { "" };
        let mut params_str: Vec<String> = self.params.iter().map(|p| p.to_string()).collect();
        if let Some((param_name, cap_spec)) = extra_cap {
            params_str.push(format!("{}: cap {}", param_name, cap_spec));
        }
        let ret_str = match &self.return_type {
            Some(ty) => format!(" -> {}", ty),
            None => String::new(),
        };
        format!(
            "{}fn {}({}){}",
            vis,
            self.name,
            params_str.join(", "),
            ret_str
        )
    }
}

/// Parâmetro de função com suporte explícito ao modificador de capacidade `cap`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FnParam {
    pub name: String,
    pub is_cap: bool,
    pub ty: TypeExpr,
    pub capability: Option<CapabilitySpec>,
    pub span: Span,
}

impl fmt::Display for FnParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_cap {
            if let Some(cap) = &self.capability {
                write!(f, "{}: cap {}", self.name, cap)
            } else {
                write!(f, "{}: cap {}", self.name, self.ty)
            }
        } else {
            write!(f, "{}: {}", self.name, self.ty)
        }
    }
}

/// Especificação formal de uma Capacidade (`cap`) declarada na assinatura de uma função.
/// Exemplos:
/// - `cap I2CBus<0x48>`
/// - `cap PWMPin<4>`
/// - `cap MutColumn<Transform>`
/// - `cap ReadColumn<Velocity>`
/// - `cap FrameArena<CollisionEvent>`
/// - `cap SqlRead<UsersTable>`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySpec {
    pub kind: CapabilityKind,
    pub domain_name: String,
    pub generic_arg: Option<CapGenericArg>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityKind {
    /// Barramento I2C com endereço de hardware estático (ex: `I2CBus<0x48>`, `I2CRead<0x48>`)
    I2CBus,
    /// Pino PWM com canal físico estático (ex: `PWMPin<4>`, `PWMWrite<Pin4>`)
    PWMPin,
    /// Pino GPIO / Barramento SPI / Canal DMA
    HardwarePort,
    /// Relógio monotônico determinístico (`MonotonicClock`, `TimeStep`)
    Clock,
    /// Acesso de leitura imutável a coluna ECS SoA (`ReadColumn<T>`)
    ReadColumn,
    /// Acesso de escrita exclusiva a coluna ECS SoA (`MutColumn<T>`)
    MutColumn,
    /// Acesso de leitura a um objeto/recurso (`Read<T>`)
    ReadResource,
    /// Acesso de escrita a um objeto/recurso (`Mut<T>`)
    MutResource,
    /// Arena de memória escopada mutável/produtora (`StaticArena`, `LevelArena`, `FrameArena<T>`, `RequestArena`, `SessionArena`)
    ScopedArena(ArenaScope),
    /// Arena de memória escopada somente-leitura/consumidora (`ReadFrameArena<T>`, `ReadRequestArena<T>`, `ReadTaskArena<T>`)
    ReadScopedArena(ArenaScope),
    /// Socket de rede (`NetSocket`, `TcpSocket`, `UdpTick`, `HttpRoute`)
    Network,
    /// Leitura de banco de dados (`SqlRead<T>`)
    SqlRead,
    /// Escrita de banco de dados (`SqlWrite<T>`)
    SqlWrite,
    /// Fila de GPU / Áudio (`GpuQueue`, `WebGpuDevice`, `AudioBuffer`)
    DeviceQueue,
    /// Capacidade customizada definida pelo domínio
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ArenaScope {
    /// Ciclo de vida de um único quadro (16.6ms) ou requisição HTTP — reset em O(1) (Rank = 1)
    FrameOrRequest = 1,
    /// Ciclo de vida de uma fase de jogo, sessão de usuário ou coluna ECS persistente (Rank = 2)
    LevelOrSession = 2,
    /// Ciclo de vida de toda a aplicação/firmware (maior duração, Rank = 3)
    Static = 3,
}

impl ArenaScope {
    pub fn rank(self) -> u8 {
        self as u8
    }

    pub fn label(self) -> &'static str {
        match self {
            ArenaScope::Static => "StaticArena(Rank=3)",
            ArenaScope::LevelOrSession => "LevelArena/PersistentColumn(Rank=2)",
            ArenaScope::FrameOrRequest => "FrameArena/RequestArena(Rank=1)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapGenericArg {
    /// Argumento de tipo (ex: `Transform` em `MutColumn<Transform>`)
    TypeName(String),
    /// Endereço de hardware ou pino constante (ex: `0x48` em `I2CBus<0x48>`, `4` em `PWMPin<4>`)
    HardwareAddress(u64),
}

impl CapabilitySpec {
    pub fn from_name_and_arg(name: &str, arg: Option<CapGenericArg>) -> Self {
        let kind = match name {
            "I2CBus" | "I2CRead" | "I2CPort" => CapabilityKind::I2CBus,
            "PWMPin" | "PWMWrite" => CapabilityKind::PWMPin,
            "GPIOPin" | "SPIBus" | "DMAChannel" | "Interrupt" => CapabilityKind::HardwarePort,
            "MonotonicClock" | "TimeStep" => CapabilityKind::Clock,
            "ReadColumn" => CapabilityKind::ReadColumn,
            "MutColumn" => CapabilityKind::MutColumn,
            "Read" => CapabilityKind::ReadResource,
            "Mut" => CapabilityKind::MutResource,
            "StaticArena" => CapabilityKind::ScopedArena(ArenaScope::Static),
            "LevelArena" | "SessionArena" => {
                CapabilityKind::ScopedArena(ArenaScope::LevelOrSession)
            }
            "FrameArena" | "RequestArena" | "TaskArena" => {
                CapabilityKind::ScopedArena(ArenaScope::FrameOrRequest)
            }
            "ReadFrameArena" | "ReadRequestArena" | "ReadTaskArena" => {
                CapabilityKind::ReadScopedArena(ArenaScope::FrameOrRequest)
            }
            "NetSocket" | "TcpSocket" | "UdpTick" | "HttpRoute" => CapabilityKind::Network,
            "SqlRead" => CapabilityKind::SqlRead,
            "SqlWrite" => CapabilityKind::SqlWrite,
            "GpuQueue" | "WebGpuDevice" | "CanvasContext" | "AudioBuffer" => {
                CapabilityKind::DeviceQueue
            }
            _ => CapabilityKind::Custom,
        };
        Self {
            kind,
            domain_name: name.to_string(),
            generic_arg: arg,
        }
    }
}

impl fmt::Display for CapabilitySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.generic_arg {
            Some(CapGenericArg::TypeName(t)) => write!(f, "{}<{}>", self.domain_name, t),
            Some(CapGenericArg::HardwareAddress(addr)) => {
                if self.domain_name.starts_with("I2C") {
                    write!(f, "{}<0x{:02X}>", self.domain_name, addr)
                } else {
                    write!(f, "{}<{}>", self.domain_name, addr)
                }
            }
            None => write!(f, "{}", self.domain_name),
        }
    }
}

/// Tipos Nativos da Linguagem RustTS (Sem Lifetimes `'a`, Sem Ponteiros Brutos, Sem `null`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Usize,
    F32,
    F64,
    Fix32,
    Bool,
    Str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathType {
    Vec2,
    Vec3,
    Vec4,
    Quat,
    Mat3,
    Mat4,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeExpr {
    Primitive(PrimitiveType),
    Math(MathType),
    /// `[T; N]` — Array contíguo de tamanho fixo na stack/struct (Zero Heap Allocation)
    Array(Box<TypeExpr>, usize),
    /// `Option<T>` — Substituto seguro para `null`/`undefined`
    Option(Box<TypeExpr>),
    /// `Result<T, E>` — Substituto seguro e explícito para `try/catch/throw`
    Result(Box<TypeExpr>, Box<TypeExpr>),
    /// `Handle<T>` — Índice de 32 bits + contador geracional de 32 bits (substitui ponteiros e `'a`)
    Handle(Box<TypeExpr>),
    /// Tipo de capacidade `cap ...`
    Capability(CapabilitySpec),
    /// Tipo nominal (`SensorConfig`, `Transform`, `Json<UserProfileDTO>`)
    Named {
        name: String,
        generics: Vec<TypeExpr>,
    },
}

impl fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeExpr::Primitive(p) => {
                let s = match p {
                    PrimitiveType::I8 => "i8",
                    PrimitiveType::I16 => "i16",
                    PrimitiveType::I32 => "i32",
                    PrimitiveType::I64 => "i64",
                    PrimitiveType::U8 => "u8",
                    PrimitiveType::U16 => "u16",
                    PrimitiveType::U32 => "u32",
                    PrimitiveType::U64 => "u64",
                    PrimitiveType::Usize => "usize",
                    PrimitiveType::F32 => "f32",
                    PrimitiveType::F64 => "f64",
                    PrimitiveType::Fix32 => "fix32",
                    PrimitiveType::Bool => "bool",
                    PrimitiveType::Str => "str",
                };
                write!(f, "{}", s)
            }
            TypeExpr::Math(m) => {
                let s = match m {
                    MathType::Vec2 => "vec2",
                    MathType::Vec3 => "vec3",
                    MathType::Vec4 => "vec4",
                    MathType::Quat => "quat",
                    MathType::Mat3 => "mat3",
                    MathType::Mat4 => "mat4",
                };
                write!(f, "{}", s)
            }
            TypeExpr::Array(inner, len) => write!(f, "[{}; {}]", inner, len),
            TypeExpr::Option(inner) => write!(f, "Option<{}>", inner),
            TypeExpr::Result(ok, err) => write!(f, "Result<{}, {}>", ok, err),
            TypeExpr::Handle(inner) => write!(f, "Handle<{}>", inner),
            TypeExpr::Capability(cap) => write!(f, "cap {}", cap),
            TypeExpr::Named { name, generics } => {
                if generics.is_empty() {
                    write!(f, "{}", name)
                } else {
                    let g: Vec<String> = generics.iter().map(|t| t.to_string()).collect();
                    write!(f, "{}<{}>", name, g.join(", "))
                }
            }
        }
    }
}

/// Declarações de Instruções (Statements) dentro de funções `.rts`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Let {
        name: String,
        is_mut: bool,
        ty: Option<TypeExpr>,
        value: Expr,
        span: Span,
    },
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
        span: Span,
    },
    Match {
        expr: Expr,
        arms: Vec<MatchArm>,
        span: Span,
    },
    ForRange {
        iterator: String,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    ExprStmt {
        expr: Expr,
        span: Span,
    },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::If { span, .. }
            | Stmt::Match { span, .. }
            | Stmt::ForRange { span, .. }
            | Stmt::While { span, .. }
            | Stmt::Break { span }
            | Stmt::Continue { span }
            | Stmt::Return { span, .. }
            | Stmt::ExprStmt { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchPattern {
    /// Ex: `HardwareFault::I2CTimeout(addr)`, `HardwareFault::SensorDisconnected`, `Ok(val)`, `Err(e)`
    Variant {
        enum_name: Option<String>,
        variant: String,
        binding: Option<String>,
    },
    /// Padrão curinga `_`
    Wildcard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
}

/// Expressões da AST da RustTS
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    IntLit(u64, Span),
    FloatLit(f64, Span),
    BoolLit(bool, Span),
    StringLit(String, Span),
    Ident(String, Span),
    /// Operação Unária (`-expr`, `!expr`)
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    /// Chamada estática / qualificada: ex: `NetworkSocket::send(...)`, `math::clamp(...)`, `Handle::null()`
    StaticCall {
        namespace: String,
        func: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Chamada de método em objeto ou capacidade: ex: `i2c.read_register_u16(0x00)`, `db.delete_by_id(id)`
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Acesso a campo de estrutura: ex: `config.target_temp_celsius`, `transforms[i].position.y`
    FieldAccess {
        receiver: Box<Expr>,
        field: String,
        span: Span,
    },
    /// Indexação de coluna ECS SoA: ex: `transforms[i]`, `velocities[i]`
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// Inicialização literal de Struct: ex: `CollisionEvent { entity_a: ..., entity_b: ..., impact_point: ... }`
    StructInit {
        struct_name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    /// Construtor de Variante de Enum ou Result/Option: ex: `Ok(current_temp)`, `Err(HardwareFault::SensorDisconnected)`
    EnumConstructor {
        enum_name: Option<String>,
        variant: String,
        payload: Option<Box<Expr>>,
        span: Span,
    },
    /// Operação Binária explícita (sem coerção oculta)
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    /// Conversão numérica explícita: ex: `(raw_data as f32)`
    Cast {
        expr: Box<Expr>,
        target_type: TypeExpr,
        span: Span,
    },
    /// Propagação explícita de erro via operador `?`
    TryOp {
        expr: Box<Expr>,
        span: Span,
    },
    /// Closure curta para mapeamento de erro: ex: `|_| HardwareFault::I2CTimeout(0x48)`
    ErrorMapClosure {
        body: Box<Expr>,
        span: Span,
    },
    /// Inicialização de Array Fixo: ex: `[10.0, 20.0, 30.0]`
    ArrayLit {
        elements: Vec<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLit(_, s)
            | Expr::FloatLit(_, s)
            | Expr::BoolLit(_, s)
            | Expr::StringLit(_, s)
            | Expr::Ident(_, s)
            | Expr::Unary { span: s, .. }
            | Expr::StaticCall { span: s, .. }
            | Expr::MethodCall { span: s, .. }
            | Expr::FieldAccess { span: s, .. }
            | Expr::Index { span: s, .. }
            | Expr::StructInit { span: s, .. }
            | Expr::EnumConstructor { span: s, .. }
            | Expr::Binary { span: s, .. }
            | Expr::Cast { span: s, .. }
            | Expr::TryOp { span: s, .. }
            | Expr::ErrorMapClosure { span: s, .. }
            | Expr::ArrayLit { span: s, .. } => *s,
        }
    }
}

