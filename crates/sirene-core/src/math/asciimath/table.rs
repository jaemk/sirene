//! The AsciiMath symbol table, as ASCIIMathTeXImg.js has it (`AMsymbols`): what each input word
//! or character is, and the TeX it becomes.

/// What kind of token a symbol is (the JavaScript's `ttype`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Ty {
    /// A symbol that is its own TeX: a letter, a number, `+`, or `\alpha`.
    Const,
    /// Takes one argument: `sqrt`, `hat`, `bb`, the standard functions.
    Unary,
    /// Takes two arguments: `frac`, `root`, `color`.
    Binary,
    /// Written between two operands: `_`, `^`, `/`.
    Infix,
    LeftBracket,
    RightBracket,
    /// `and`, `or`, `if`: a word with a quad on each side.
    Space,
    /// `sum`, `lim`: a symbol that takes limits. Read as a constant, as the translator does.
    UnderOver,
    /// A word that stands for other AsciiMath: `iff` is `<=>`.
    Definition,
    /// `|`: an absolute value bar, or a divider.
    LeftRight,
    /// `text`, `mbox`, `"..."`, `mspace`.
    Text,
    /// A symbol the table gives no type (`''`): written in braces.
    Unset,
}

/// The symbols the translator treats on their own, by input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Id {
    None,
    Sqrt,
    Cancel,
    Root,
    Color,
    Slash,
    Sub,
    Sup,
    Quote,
    Mspace,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Sym {
    pub input: &'static str,
    /// The TeX name when it differs from the input; the name is also an input, as a TeX alias.
    pub tex: Option<&'static str>,
    /// For a definition, the AsciiMath it stands for.
    pub out: &'static str,
    pub ttype: Ty,
    pub id: Id,
    /// Written as it is, without a backslash.
    pub val: bool,
    /// A function: `sin x` takes the next expression.
    pub func: bool,
    /// An accent: its TeX has no braces around it.
    pub acc: bool,
    /// A bracket that is not drawn.
    pub invisible: bool,
    /// The TeX name is not also an input.
    pub noalias: bool,
    /// The delimiters `abs`, `norm`, `floor`, and `ceil` draw around their argument.
    pub lr: Option<(&'static str, &'static str)>,
}

impl Sym {
    const fn new(input: &'static str, tex: Option<&'static str>, ttype: Ty) -> Sym {
        Sym {
            input,
            tex,
            out: "",
            ttype,
            id: Id::None,
            val: false,
            func: false,
            acc: false,
            invisible: false,
            noalias: false,
            lr: None,
        }
    }

    /// The symbol the TeX name of `self` is as an input (`AMinitSymbols`): the same type and
    /// accent flag, written as a plain command.
    pub fn alias(&self) -> Option<Sym> {
        let name = self.tex?;
        if self.noalias {
            return None;
        }
        let mut s = Sym::new(name, None, self.ttype);
        s.acc = self.acc;
        Some(s)
    }

    const fn val(mut self) -> Sym {
        self.val = true;
        self
    }

    const fn func(mut self) -> Sym {
        self.func = true;
        self
    }

    const fn acc(mut self) -> Sym {
        self.acc = true;
        self
    }

    const fn invisible(mut self) -> Sym {
        self.invisible = true;
        self
    }

    const fn noalias(mut self) -> Sym {
        self.noalias = true;
        self
    }

    const fn id(mut self, id: Id) -> Sym {
        self.id = id;
        self
    }

    const fn lr(mut self, left: &'static str, right: &'static str) -> Sym {
        self.lr = Some((left, right));
        self
    }
}

/// The symbol for text the table does not have: written as it is.
pub(super) const fn plain() -> Sym {
    Sym::new("", None, Ty::Const).val()
}

/// A constant whose TeX is `\input`.
const fn c(input: &'static str) -> Sym {
    Sym::new(input, None, Ty::Const)
}

/// A constant whose TeX is `\tex`.
const fn t(input: &'static str, tex: &'static str) -> Sym {
    Sym::new(input, Some(tex), Ty::Const)
}

/// A symbol with limits, `\input`.
const fn uo(input: &'static str) -> Sym {
    Sym::new(input, None, Ty::UnderOver)
}

/// A symbol with limits, `\tex`.
const fn uot(input: &'static str, tex: &'static str) -> Sym {
    Sym::new(input, Some(tex), Ty::UnderOver)
}

/// A function: `\input` with the next expression as its argument.
const fn f(input: &'static str) -> Sym {
    Sym::new(input, None, Ty::Unary).func()
}

/// A function KaTeX has no command for: set as an operator name.
const fn fo(input: &'static str, tex: &'static str) -> Sym {
    Sym::new(input, Some(tex), Ty::Unary).func().noalias()
}

/// A unary command with TeX name `tex`.
const fn u(input: &'static str, tex: Option<&'static str>) -> Sym {
    Sym::new(input, tex, Ty::Unary)
}

/// A word standing for other AsciiMath.
const fn d(input: &'static str, out: &'static str) -> Sym {
    let mut s = Sym::new(input, None, Ty::Definition);
    s.out = out;
    s
}

const fn lb(input: &'static str, tex: Option<&'static str>) -> Sym {
    Sym::new(input, tex, Ty::LeftBracket)
}

const fn rb(input: &'static str, tex: Option<&'static str>) -> Sym {
    Sym::new(input, tex, Ty::RightBracket)
}

/// `AMsymbols`, in the JavaScript's order: where two inputs are the same, the first one wins.
pub(super) static SYMS: &[Sym] = &[
    // Greek letters.
    c("alpha"),
    c("beta"),
    c("chi"),
    c("delta"),
    c("Delta"),
    t("epsi", "epsilon"),
    c("varepsilon"),
    c("eta"),
    c("gamma"),
    c("Gamma"),
    c("iota"),
    c("kappa"),
    c("lambda"),
    c("Lambda"),
    d("lamda", "lambda"),
    d("Lamda", "Lambda"),
    c("mu"),
    c("nu"),
    c("omega"),
    c("Omega"),
    c("phi"),
    c("varphi"),
    c("Phi"),
    c("pi"),
    c("Pi"),
    c("psi"),
    c("Psi"),
    c("rho"),
    c("sigma"),
    c("Sigma"),
    c("tau"),
    c("theta"),
    c("vartheta"),
    c("Theta"),
    c("upsilon"),
    c("xi"),
    c("Xi"),
    c("zeta"),
    // Binary operation symbols.
    t("*", "cdot"),
    t("**", "ast"),
    t("***", "star"),
    t("//", "/").val().noalias(),
    t("\\\\", "backslash"),
    c("setminus"),
    t("xx", "times"),
    t("|><", "ltimes"),
    t("><|", "rtimes"),
    t("|><|", "bowtie"),
    t("-:", "div"),
    d("divide", "-:"),
    t("@", "circ"),
    t("o+", "oplus"),
    t("o-", "ominus"),
    t("ox", "otimes"),
    t("o.", "odot"),
    uo("sum"),
    uo("prod"),
    t("^^", "wedge"),
    uot("^^^", "bigwedge"),
    t("vv", "vee"),
    uot("vvv", "bigvee"),
    t("nn", "cap"),
    uot("nnn", "bigcap"),
    t("uu", "cup"),
    uot("uuu", "bigcup"),
    Sym::new("overset", None, Ty::Binary),
    Sym::new("underset", None, Ty::Binary),
    t("dag", "dagger"),
    t("ddag", "ddagger"),
    // Binary relation symbols.
    t("!=", "ne"),
    c(":="),
    c("lt"),
    t("<=", "le"),
    t("lt=", "leq"),
    c("gt"),
    t("mlt", "ll"),
    t(">=", "ge"),
    t("gt=", "geq"),
    t("mgt", "gg"),
    t("-<", "prec"),
    t("-lt", "prec"),
    t(">-", "succ"),
    t("-<=", "preceq"),
    t(">-=", "succeq"),
    c("in"),
    t("!in", "notin"),
    t("sub", "subset"),
    t("!sub", "not\\subset"),
    d("notsubset", "!sub"),
    t("sup", "supset"),
    t("!sup", "not\\supset"),
    d("notsupset", "!sup"),
    t("sube", "subseteq"),
    t("!sube", "not\\subseteq"),
    d("notsubseteq", "!sube"),
    t("supe", "supseteq"),
    t("!supe", "not\\supseteq"),
    d("notsupseteq", "!supe"),
    t("-=", "equiv"),
    t("!-=", "not\\equiv"),
    d("notequiv", "!-="),
    t("~=", "stackrel{\\sim}{=}").noalias(),
    d("cong", "~="),
    t("~~", "approx"),
    t("~", "sim"),
    t("prop", "propto"),
    // Logical symbols.
    Sym::new("and", None, Ty::Space),
    Sym::new("or", None, Ty::Space),
    t("not", "neg"),
    t("=>", "Rightarrow"),
    d("implies", "=>"),
    Sym::new("if", None, Ty::Space),
    t("<=>", "Leftrightarrow"),
    d("iff", "<=>"),
    t("AA", "forall"),
    t("EE", "exists"),
    t("_|_", "bot"),
    t("TT", "top"),
    t("|--", "vdash"),
    t("|==", "models"),
    // Grouping brackets.
    lb("(", None).val(),
    rb(")", None).val(),
    lb("[", None).val(),
    rb("]", None).val(),
    lb("left(", Some("(")).val().noalias(),
    rb("right)", Some(")")).val().noalias(),
    lb("left[", Some("[")).val().noalias(),
    rb("right]", Some("]")).val().noalias(),
    lb("{", Some("lbrace")),
    rb("}", Some("rbrace")),
    Sym::new("|", None, Ty::LeftRight).val(),
    lb("|:", Some("|")).val().noalias(),
    rb(":|", Some("|")).val().noalias(),
    t(":|:", "|").val().noalias(),
    lb("(:", Some("langle")),
    rb(":)", Some("rangle")),
    lb("<<", Some("langle")),
    rb(">>", Some("rangle")),
    lb("{:", None).invisible(),
    rb(":}", None).invisible(),
    // Miscellaneous symbols.
    c("int"),
    d("dx", "{:d x:}"),
    d("dy", "{:d y:}"),
    d("dz", "{:d z:}"),
    d("dt", "{:d t:}"),
    c("oint"),
    t("del", "partial"),
    t("grad", "nabla"),
    t("+-", "pm"),
    t("-+", "mp"),
    t("O/", "emptyset"),
    t("oo", "infty"),
    c("aleph"),
    t("...", "ldots"),
    t(":.", "therefore"),
    t(":'", "because"),
    t("/_", "angle"),
    t("/_\\", "triangle"),
    c("\\ ").val(),
    c("frown"),
    t("%", "%").noalias(),
    c("quad"),
    c("qquad"),
    c("enspace"),
    c("thinspace"),
    Sym::new("mspace", None, Ty::Text).id(Id::Mspace),
    c("cdots"),
    c("vdots"),
    c("ddots"),
    c("diamond"),
    t("square", "square"),
    t("|__", "lfloor"),
    t("__|", "rfloor"),
    t("|~", "lceil"),
    d("lceiling", "|~"),
    t("~|", "rceil"),
    d("rceiling", "~|"),
    t("CC", "mathbb{C}").noalias(),
    t("NN", "mathbb{N}").noalias(),
    t("QQ", "mathbb{Q}").noalias(),
    t("RR", "mathbb{R}").noalias(),
    t("ZZ", "mathbb{Z}").noalias(),
    u("f", None).func().val(),
    u("g", None).func().val(),
    c("hbar"),
    Sym::new("''", None, Ty::Unset).val(),
    Sym::new("'''", None, Ty::Unset).val(),
    Sym::new("''''", None, Ty::Unset).val(),
    // Standard functions.
    uo("lim"),
    uot("Lim", "operatorname{Lim}").noalias(),
    f("sin"),
    f("cos"),
    f("tan"),
    f("arcsin"),
    f("arccos"),
    f("arctan"),
    fo("arcsec", "operatorname{arcsec}"),
    fo("arccsc", "operatorname{arccsc}"),
    fo("arccot", "operatorname{arccot}"),
    f("sinh"),
    f("cosh"),
    f("tanh"),
    f("cot"),
    f("coth"),
    fo("sech", "operatorname{sech}"),
    fo("csch", "operatorname{csch}"),
    f("sec"),
    f("csc"),
    f("log"),
    f("ln"),
    u("abs", None).noalias().lr("|", "|"),
    u("norm", None).noalias().lr("\\|", "\\|"),
    u("floor", None).noalias().lr("\\lfloor", "\\rfloor"),
    u("ceil", None).noalias().lr("\\lceil", "\\rceil"),
    fo("Sin", "operatorname{Sin}"),
    fo("Cos", "operatorname{Cos}"),
    fo("Tan", "operatorname{Tan}"),
    fo("Arcsin", "operatorname{Arcsin}"),
    fo("Arccos", "operatorname{Arccos}"),
    fo("Arctan", "operatorname{Arctan}"),
    fo("Sinh", "operatorname{Sinh}"),
    fo("Cosh", "operatorname{Cosh}"),
    fo("Tanh", "operatorname{Tanh}"),
    fo("Cot", "operatorname{Cot}"),
    fo("Sec", "operatorname{Sec}"),
    fo("Csc", "operatorname{Csc}"),
    fo("Log", "operatorname{Log}"),
    fo("Ln", "operatorname{Ln}"),
    u("Abs", None).noalias().lr("|", "|"),
    f("det"),
    f("exp"),
    c("dim"),
    t("mod", "text{mod}").noalias(),
    f("gcd"),
    Sym::new("lcm", Some("text{lcm}"), Ty::Unary)
        .func()
        .noalias(),
    t("lub", "operatorname{lub}").noalias(),
    t("glb", "operatorname{glb}").noalias(),
    uo("min"),
    uo("max"),
    // Arrows.
    t("uarr", "uparrow"),
    t("darr", "downarrow"),
    t("rarr", "rightarrow"),
    t("->", "to"),
    t(">->", "rightarrowtail"),
    t("->>", "twoheadrightarrow"),
    t(">->>", "twoheadrightarrow"),
    t("|->", "mapsto"),
    t("larr", "leftarrow"),
    t("harr", "leftrightarrow"),
    t("rArr", "Rightarrow"),
    t("lArr", "Leftarrow"),
    t("dArr", "Downarrow"),
    t("hArr", "Leftrightarrow"),
    c("rightleftharpoons"),
    // Commands with arguments.
    u("sqrt", None).id(Id::Sqrt),
    Sym::new("root", None, Ty::Binary).id(Id::Root),
    Sym::new("frac", None, Ty::Binary),
    Sym::new("/", None, Ty::Infix).id(Id::Slash),
    Sym::new("stackrel", None, Ty::Binary),
    Sym::new("_", None, Ty::Infix).id(Id::Sub),
    Sym::new("^", None, Ty::Infix).id(Id::Sup),
    u("cancel", None).id(Id::Cancel),
    u("Sqrt", Some("sqrt")).id(Id::Sqrt).noalias(),
    u("hat", None).acc(),
    u("bar", Some("overline")).acc(),
    u("vec", None).acc(),
    u("tilde", None).acc(),
    u("dot", None).acc(),
    u("ddot", None).acc(),
    u("overarc", Some("stackrel{\\frown}")).acc().noalias(),
    u("overparen", Some("stackrel{\\frown}")).acc().noalias(),
    u("ul", Some("underline")).acc(),
    u("ubrace", Some("underbrace")).acc(),
    u("obrace", Some("overbrace")).acc(),
    Sym::new("text", None, Ty::Text),
    Sym::new("mbox", None, Ty::Text),
    Sym::new("\"", None, Ty::Text).id(Id::Quote),
    Sym::new("color", None, Ty::Binary).id(Id::Color),
    u("bb", Some("mathbf")),
    u("sf", Some("mathsf")),
    u("bbb", Some("mathbb")),
    u("cc", Some("mathcal")),
    u("tt", Some("mathtt")),
    u("fr", Some("mathfrak")),
    u("italic", Some("mathit")),
    d("sfit", "sf italic"),
    d("bbit", "bb italic"),
    d("bbsfit", "bb sf italic"),
    d("bold", "bb"),
];
