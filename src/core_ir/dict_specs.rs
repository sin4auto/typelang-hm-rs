use std::borrow::Cow;
use std::collections::HashMap;

use once_cell::sync::Lazy;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignaturePattern {
    SameTypeBinary,
    SameTypeBinaryBoolResult,
    BoolBinary,
    BoolUnary,
    FromIntToSame,
}

impl SignaturePattern {
    pub const fn generic_signature(&self) -> &'static str {
        match self {
            SignaturePattern::SameTypeBinary => "a -> a -> a",
            SignaturePattern::SameTypeBinaryBoolResult => "a -> a -> Bool",
            SignaturePattern::BoolBinary => "Bool -> Bool -> Bool",
            SignaturePattern::BoolUnary => "Bool -> Bool",
            SignaturePattern::FromIntToSame => "Int -> a",
        }
    }

    pub fn instantiate(&self, ty: &str) -> Cow<'static, str> {
        match self {
            SignaturePattern::SameTypeBinary => Cow::Owned(format!("{ty} -> {ty} -> {ty}")),
            SignaturePattern::SameTypeBinaryBoolResult => {
                Cow::Owned(format!("{ty} -> {ty} -> Bool"))
            }
            SignaturePattern::BoolBinary => Cow::Borrowed("Bool -> Bool -> Bool"),
            SignaturePattern::BoolUnary => Cow::Borrowed("Bool -> Bool"),
            SignaturePattern::FromIntToSame => Cow::Owned(format!("Int -> {ty}")),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MethodSpec {
    pub name: &'static str,
    pub method_id: u64,
    pub pattern: SignaturePattern,
}

#[derive(Clone, Copy, Debug)]
pub struct ClassMethodSet {
    pub classname: &'static str,
    pub methods: &'static [MethodSpec],
}

macro_rules! define_class_methods {
    (
        $(
            $class:literal => {
                $( { $name:literal, $id:expr, $pattern:expr } ),+ $(,)?
            }
        ),+ $(,)?
    ) => {
        pub const CLASS_METHODS: &[ClassMethodSet] = &[
            $(
                ClassMethodSet {
                    classname: $class,
                    methods: &[
                        $(
                            MethodSpec {
                                name: $name,
                                method_id: $id,
                                pattern: $pattern,
                            },
                        )+
                    ],
                },
            )+
        ];
    };
}

define_class_methods! {
    "Num" => {
        { "add", 0, SignaturePattern::SameTypeBinary },
        { "sub", 1, SignaturePattern::SameTypeBinary },
        { "mul", 2, SignaturePattern::SameTypeBinary },
        { "fromInt", 3, SignaturePattern::FromIntToSame },
    },
    "Fractional" => {
        { "div", 0, SignaturePattern::SameTypeBinary },
    },
    "Integral" => {
        { "div", 0, SignaturePattern::SameTypeBinary },
        { "mod", 1, SignaturePattern::SameTypeBinary },
    },
    "Eq" => {
        { "eq", 0, SignaturePattern::SameTypeBinaryBoolResult },
        { "neq", 1, SignaturePattern::SameTypeBinaryBoolResult },
    },
    "Ord" => {
        { "lt", 0, SignaturePattern::SameTypeBinaryBoolResult },
        { "le", 1, SignaturePattern::SameTypeBinaryBoolResult },
        { "gt", 2, SignaturePattern::SameTypeBinaryBoolResult },
        { "ge", 3, SignaturePattern::SameTypeBinaryBoolResult },
    },
    "BoolLogic" => {
        { "and", 0, SignaturePattern::BoolBinary },
        { "or", 1, SignaturePattern::BoolBinary },
        { "not", 2, SignaturePattern::BoolUnary },
    },
}

static METHOD_SPECS_BY_CLASS_AND_NAME: Lazy<
    HashMap<&'static str, HashMap<&'static str, &'static MethodSpec>>,
> = Lazy::new(|| {
    let mut map: HashMap<&'static str, HashMap<&'static str, &'static MethodSpec>> = HashMap::new();
    for class in CLASS_METHODS {
        let inner = map.entry(class.classname).or_default();
        for method in class.methods {
            inner.insert(method.name, method);
        }
    }
    map
});

pub fn methods_for_class(classname: &str) -> Option<&'static [MethodSpec]> {
    CLASS_METHODS
        .iter()
        .find(|set| set.classname == classname)
        .map(|set| set.methods)
}

pub fn lookup_method_spec(classname: &str, method: &str) -> Option<&'static MethodSpec> {
    METHOD_SPECS_BY_CLASS_AND_NAME
        .get(classname)
        .and_then(|methods| methods.get(method))
        .copied()
}
