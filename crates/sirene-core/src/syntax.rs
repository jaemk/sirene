//! Syntax kinds and rowan tree types shared by all diagram parsers.

macro_rules! syntax_kinds {
    ($($name:ident),* $(,)?) => {
        /// Kind of a token or node in the lossless syntax tree.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
        pub enum SyntaxKind {
            $($name),*
        }

        impl SyntaxKind {
            const ALL: &'static [SyntaxKind] = &[$(SyntaxKind::$name),*];
        }
    };
}

syntax_kinds! {
    // Trivia and separators.
    WHITESPACE,
    NEWLINE,
    COMMENT,
    DIRECTIVE,
    FRONTMATTER,
    SEMI,

    // Keywords.
    GRAPH_KW,
    SUBGRAPH_KW,
    END_KW,
    DIRECTION_KW,
    CLASSDEF_KW,
    CLASS_KW,
    STYLE_KW,
    LINKSTYLE_KW,
    CLICK_KW,
    HREF_KW,
    CALL_KW,
    DEFAULT_KW,
    INTERPOLATE_KW,
    ACC_TITLE_KW,
    ACC_DESCR_KW,
    STATE_KW,
    AS_KW,
    NOTE_KW,
    OF_KW,
    NAMESPACE_KW,
    CSSCLASS_KW,
    FOR_KW,

    // Other tokens.
    DIR,
    IDENT,
    NUMBER,
    COMMA,
    COLON,
    AMP,
    TRIPLE_COLON,
    AT,
    SHAPE_OPEN,
    SHAPE_CLOSE,
    TEXT,
    STRING,
    MD_STRING,
    PIPE,
    STROKE,
    STROKE_OPEN,
    META_OPEN,
    META_BODY,
    META_CLOSE,
    STYLE_TEXT,
    L_PAREN,
    R_PAREN,
    CALLBACK_ARGS,
    L_BRACE,
    R_BRACE,
    ACC_TEXT,
    LINK_TARGET,
    // State diagrams: `left` or `right` in a note, `[*]`, `<<fork>>`, and `--` between
    // concurrent regions.
    SIDE,
    STAR,
    STEREOTYPE,
    REGION_SEP,
    // Class diagrams: `~T~`, a relation like `<|--`, and a member line in a class body.
    GENERIC,
    RELATION,
    MEMBER,
    // ER diagrams: an attribute key list such as `PK, FK`.
    KEYS,
    // Git graphs: `commit`, `branch`, `checkout`, `switch`, `merge`, or `cherry-pick`, and an
    // attribute name such as `id` or `tag`.
    GIT_KW,
    ATTR_KEY,
    // Timelines: `title` and `section`, a time period's text, and an event's.
    TITLE_KW,
    SECTION_KW,
    PERIOD,
    EVENT,
    // Sequence diagrams: a statement keyword such as `participant` or `loop`, a message arrow
    // such as `->>`, and the `+` or `-` after it.
    SEQ_KW,
    ARROW,
    ACTIVATION,
    // Pie charts: `showData` after the header keyword.
    SHOW_DATA_KW,
    // Gantt charts: a setting keyword such as `dateFormat`, and a task's name and data.
    SETTING_KW,
    TASK_NAME,
    TASK_DATA,
    // Mindmaps: `::icon` before a node's icon.
    ICON_KW,
    // Quadrant charts: `x-axis` or `y-axis`, `quadrant-1` to `quadrant-4`, and the brackets
    // around a point's coordinates.
    AXIS_KW,
    QUADRANT_KW,
    L_BRACKET,
    R_BRACKET,
    // XY charts: `line` or `bar`, and `horizontal` or `vertical` after the header keyword.
    SERIES_KW,
    ORIENTATION,
    // Requirement diagrams: a requirement type or `element`.
    REQ_KW,
    // Packet diagrams: `+` before a bit count and `-` in a bit range.
    PLUS,
    DASH,
    // ZenUML: `=` before a call, `.` before a method, and a keyword such as `new`, `if`, or
    // `return`.
    EQ,
    DOT,
    ZEN_KW,
    // Info diagrams: `showInfo`.
    SHOW_INFO_KW,
    // Venn diagrams: `set`, `union`, or `text`.
    VENN_KW,
    // Wardley maps: a statement keyword such as `component` or `evolve`, and a link's flow
    // marker such as `+>` or `+'label'<>`.
    WARDLEY_KW,
    PORT,
    // Tree views: a box-drawing prefix such as `│   ├── `, `icon(name)`, and `## text`.
    TREE_BRANCH,
    TREE_ICON,
    TREE_DESC,
    // Use case diagrams: `actor`, `systemBoundary`, or `json`, and a JSON table's object.
    USECASE_KW,
    JSON_BODY,
    // Railroad diagrams: a constructor such as `choice`, an operator such as `|` or `*`, an
    // ABNF repeat such as `1*` or `3`, an ABNF number such as `%x30-39`, and an EBNF special
    // sequence `? text ?`.
    RAILROAD_KW,
    RR_OP,
    RR_REPEAT,
    RR_NUMVAL,
    RR_SPECIAL,
    // Cynefin diagrams: a domain name such as `complex`.
    CYNEFIN_KW,
    // Event modeling diagrams: a statement keyword such as `tf`, `data`, or `given`, an entity
    // type such as `cmd`, data in braces or quotes, and a data type such as `` `json` ``.
    EM_KW,
    EM_TYPE,
    EM_DATA,
    EM_DATA_TYPE,
    // Agentflow diagrams: the `global` and `connector` keywords (`flow` is a `SUBGRAPH_KW`).
    GLOBAL_KW,
    CONNECTOR_KW,
    // DOT: a keyword such as `digraph` or `node` (any case), and an HTML string `<...>`.
    DOT_KW,
    HTML_STRING,
    ERROR_TOKEN,

    // Nodes.
    ROOT,
    HEADER,
    VERTEX_STMT,
    NODE_GROUP,
    NODE,
    SHAPE,
    LABEL,
    CLASS_SHORTHAND,
    METADATA,
    EDGE,
    EDGE_ID,
    EDGE_LABEL,
    SUBGRAPH,
    SUBGRAPH_HEADER,
    DIRECTION_STMT,
    CLASSDEF_STMT,
    CLASS_STMT,
    STYLE_STMT,
    LINKSTYLE_STMT,
    CLICK_STMT,
    ACC_TITLE_STMT,
    ACC_DESCR_STMT,
    ID_LIST,
    STATE_REF,
    STATE_STMT,
    TRANSITION_STMT,
    DESCRIPTION_STMT,
    COMPOSITE,
    NOTE_STMT,
    REGION_STMT,
    CLASS_DECL,
    CLASS_REF,
    MEMBER_STMT,
    RELATION_STMT,
    NAMESPACE,
    CSSCLASS_STMT,
    ANNOTATION_STMT,
    ENTITY,
    ENTITY_REF,
    ATTRIBUTE,
    GIT_STMT,
    ATTR,
    TITLE_STMT,
    SECTION_STMT,
    PERIOD_STMT,
    EVENTS_STMT,
    PARTICIPANT_STMT,
    MESSAGE_STMT,
    SEQ_STMT,
    SLICE_STMT,
    SETTING_STMT,
    TASK_STMT,
    ICON_STMT,
    AXIS_STMT,
    QUADRANT_STMT,
    POINT_STMT,
    SERIES_STMT,
    REQUIREMENT,
    FIELD,
    // Radar charts: an axis or curve with its optional label, and one of a curve's values.
    ENTRY,
    VALUE,
    // Sankey diagrams: a `source,target,value` row.
    FLOW,
    // ZenUML: a sync call or creation, a fragment such as `if` ... `else`, and one of its
    // branches.
    CALL,
    FRAGMENT,
    BRANCH,
    // Info diagrams: `showInfo` on its own line.
    SHOW_INFO_STMT,
    // Venn diagrams: a `set` or `union`, and a `text` node.
    SET_STMT,
    TEXT_STMT,
    // Wardley maps: a keyword statement, a link, a `pipeline` block, an evolution stage, a
    // `[visibility, evolution]` pair, and a `(build)` or `(inertia)` decorator.
    WARDLEY_STMT,
    LINK_STMT,
    PIPELINE,
    STAGE,
    COORDS,
    DECORATOR,
    // Use case diagrams: `id@{ ... }` on its own line, and a `json` table.
    META_STMT,
    JSON_STMT,
    // Railroad diagrams: a rule, alternatives, a sequence, an element with its operators, a
    // bracketed group, and a constructor call.
    RULE,
    RR_CHOICE,
    RR_SEQ,
    RR_TERM,
    RR_GROUP,
    RR_CALL,
    // Cynefin diagrams: a domain block's opening line, and a line of quoted items.
    DOMAIN_STMT,
    ITEM_STMT,
    // Event modeling diagrams: a time or reset frame, a data block, and a given/when/then
    // scenario.
    FRAME_STMT,
    DATA_STMT,
    GWT_STMT,
    // Agentflow: a `global ... end` block and a `connector` declaration.
    GLOBAL,
    CONNECTOR_STMT,
    // DOT: a graph, a node, edge, or attribute statement, an `a = b` assignment, an attribute
    // list `[...]`, and an ID (with its `+` concatenations). Subgraphs are `SUBGRAPH`, an
    // attribute `ATTR`, and a node ID with its port `NODE`.
    DOT_GRAPH,
    DOT_NODE_STMT,
    DOT_EDGE_STMT,
    DOT_ATTR_STMT,
    DOT_ASSIGN,
    DOT_ATTR_LIST,
    DOT_ID,
    ERROR,
}

impl SyntaxKind {
    /// Whether this kind is whitespace, a newline, a comment, or a directive.
    pub fn is_trivia(self) -> bool {
        matches!(self, WHITESPACE | NEWLINE | COMMENT | DIRECTIVE)
    }
}

use SyntaxKind::*;

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

/// The rowan language tag for Mermaid syntax trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mermaid {}

impl rowan::Language for Mermaid {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind::ALL[usize::from(raw.0)]
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<Mermaid>;
pub type SyntaxToken = rowan::SyntaxToken<Mermaid>;
pub type SyntaxElement = rowan::SyntaxElement<Mermaid>;

#[cfg(test)]
mod tests {
    use super::*;
    use rowan::Language;

    #[test]
    fn raw_kinds_round_trip() {
        for &kind in SyntaxKind::ALL {
            assert_eq!(Mermaid::kind_from_raw(Mermaid::kind_to_raw(kind)), kind);
        }
    }

    #[test]
    fn trivia_kinds() {
        assert!(WHITESPACE.is_trivia());
        assert!(NEWLINE.is_trivia());
        assert!(COMMENT.is_trivia());
        assert!(DIRECTIVE.is_trivia());
        assert!(!SEMI.is_trivia());
        assert!(!IDENT.is_trivia());
    }
}
