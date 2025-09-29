// パス: src/typesys.rs
// 役割: 型表現・置換・制約操作など型システムの基盤を提供する
// 意図: 型推論・デフォルト化・整形表示を支える共通ユーティリティを集約する
// 関連ファイル: src/infer.rs, src/ast.rs, tests/typesys_additional.rs
//! 型システム基盤モジュール
//!
//! - `Type` や `Constraint` など、型推論で扱うコアデータ構造を定義する。
//! - 置換操作・自由型変数計算・単一化といった基本演算を提供する。
//! - 型の整形表示や単純なデフォルト化もこのモジュールで完結する。

use std::collections::{HashMap, HashSet};

/// 型変数を一意に識別する ID コンテナ。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TVar {
    pub id: i64,
}

/// 型コンストラクタ名を保持するレコード。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TCon {
    pub name: String,
}

/// 型適用 `func arg` を記述するノード。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TApp {
    pub func: Box<Type>,
    pub arg: Box<Type>,
}

/// 関数型の引数・戻り値ペアを保持するノード。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TFun {
    pub arg: Box<Type>,
    pub ret: Box<Type>,
}

/// タプル型を構成する要素群を表すノード。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TTuple {
    pub items: Vec<Type>,
}

/// 型システムで扱う各種型バリアント。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    TVar(TVar),
    TCon(TCon),
    TApp(TApp),
    TFun(TFun),
    TTuple(TTuple),
}

/// `[]` コンストラクタを用いてリスト型を構築するヘルパー関数。
pub fn t_list(elem: Type) -> Type {
    Type::TApp(TApp {
        func: Box::new(Type::TCon(TCon { name: "[]".into() })),
        arg: Box::new(elem),
    })
}

/// `String` 型（`[Char]`）を構築するヘルパー関数。
pub fn t_string() -> Type {
    t_list(Type::TCon(TCon {
        name: "Char".into(),
    }))
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// 型クラス名と対象型を関連付ける制約。
pub struct Constraint {
    pub classname: String,
    pub r#type: Type,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// 制約と型本体を組み合わせた Qualified Type。
pub struct QualType {
    pub constraints: Vec<Constraint>,
    pub r#type: Type,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// 全称型変数を束縛する型スキーム。
pub struct Scheme {
    pub vars: Vec<TVar>,
    pub qual: QualType,
}

pub type Subst = HashMap<i64, Type>;

/// 与えられた型に現れる自由型変数 ID の集合を計算する。
pub fn ftv(t: &Type) -> HashSet<i64> {
    match t {
        Type::TVar(TVar { id }) => HashSet::from([*id]),
        Type::TCon(_) => HashSet::new(),
        Type::TApp(TApp { func, arg }) => {
            let mut s = ftv(func);
            s.extend(ftv(arg));
            s
        }
        Type::TFun(TFun { arg, ret }) => {
            let mut s = ftv(arg);
            s.extend(ftv(ret));
            s
        }
        Type::TTuple(TTuple { items }) => {
            let mut s = HashSet::new();
            for it in items {
                s.extend(ftv(it));
            }
            s
        }
    }
}

/// 単一の型へ置換マップを適用する。
pub fn apply_subst_t(s: &Subst, t: &Type) -> Type {
    match t {
        Type::TVar(TVar { id }) => s.get(id).cloned().unwrap_or_else(|| t.clone()),
        Type::TCon(_) => t.clone(),
        Type::TApp(TApp { func, arg }) => Type::TApp(TApp {
            func: Box::new(apply_subst_t(s, func)),
            arg: Box::new(apply_subst_t(s, arg)),
        }),
        Type::TFun(TFun { arg, ret }) => Type::TFun(TFun {
            arg: Box::new(apply_subst_t(s, arg)),
            ret: Box::new(apply_subst_t(s, ret)),
        }),
        Type::TTuple(TTuple { items }) => Type::TTuple(TTuple {
            items: items.iter().map(|it| apply_subst_t(s, it)).collect(),
        }),
    }
}

/// 制約に置換を適用し、対象型を更新する。
pub fn apply_subst_c(s: &Subst, c: &Constraint) -> Constraint {
    Constraint {
        classname: c.classname.clone(),
        r#type: apply_subst_t(s, &c.r#type),
    }
}
/// Qualified Type 全体に置換を適用する。
pub fn apply_subst_q(s: &Subst, q: &QualType) -> QualType {
    QualType {
        constraints: q.constraints.iter().map(|c| apply_subst_c(s, c)).collect(),
        r#type: apply_subst_t(s, &q.r#type),
    }
}
/// 型スキームへ置換を適用する（束縛変数は除外する）。
pub fn apply_subst_s(s: &Subst, sc: &Scheme) -> Scheme {
    let bound: HashSet<i64> = sc.vars.iter().map(|tv| tv.id).collect();
    let s2: Subst = s
        .iter()
        .filter(|(k, _)| !bound.contains(k))
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    Scheme {
        vars: sc.vars.clone(),
        qual: apply_subst_q(&s2, &sc.qual),
    }
}

/// 2つの置換を合成する。
pub fn compose(a: &Subst, b: &Subst) -> Subst {
    // a ∘ b（先に b を適用し、その結果に a を重ねる）
    let mut out: Subst = b.iter().map(|(k, v)| (*k, apply_subst_t(a, v))).collect();
    for (k, v) in a {
        out.insert(*k, v.clone());
    }
    out
}

#[derive(Clone, Debug)]
/// 名前から型スキームへのマッピングを保持する環境。
pub struct TypeEnv {
    pub env: HashMap<String, Scheme>,
}
/// `TypeEnv` を操作するための基本メソッド群。
impl TypeEnv {
    /// 空の環境を生成する。
    pub fn new() -> Self {
        Self {
            env: HashMap::new(),
        }
    }
    /// 環境を深く複製する。
    pub fn clone_env(&self) -> Self {
        Self {
            env: self.env.clone(),
        }
    }
    /// 名前と型スキームを環境へ追加する。
    pub fn extend(&mut self, name: impl Into<String>, sch: Scheme) {
        self.env.insert(name.into(), sch);
    }
    /// 名前から型スキームを検索する。
    pub fn lookup(&self, name: &str) -> Option<&Scheme> {
        self.env.get(name)
    }
}

/// `TypeEnv` の `Default` 実装。
impl Default for TypeEnv {
    /// 空の環境で初期化する。
    fn default() -> Self {
        Self::new()
    }
}

/// 型環境全体で自由な型変数 ID を収集する。
fn env_ftv(env: &TypeEnv) -> HashSet<i64> {
    let mut s = HashSet::new();
    for sch in env.env.values() {
        let mut tvars = ftv(&sch.qual.r#type);
        for tv in &sch.vars {
            tvars.remove(&tv.id);
        }
        s.extend(tvars);
    }
    s
}

/// 指定した制約と型本体から `QualType` を構築する。
pub fn qualify(t: Type, constraints: Vec<Constraint>) -> QualType {
    QualType {
        constraints,
        r#type: t,
    }
}

/// 未使用の型変数 ID を順番に払い出す供給器。
#[derive(Clone, Debug)]
pub struct TVarSupply {
    next: i64,
}
/// `TVarSupply` の操作メソッド。
impl TVarSupply {
    /// カウンタを 0 に初期化する。
    pub fn new() -> Self {
        Self { next: 0 }
    }
    /// 未使用の型変数を生成する。
    pub fn fresh(&mut self) -> TVar {
        let id = self.next;
        self.next += 1;
        TVar { id }
    }
}

/// `TVarSupply` の `Default` 実装。
impl Default for TVarSupply {
    /// `new` と同等に初期化する。
    fn default() -> Self {
        Self::new()
    }
}

/// 環境外の型変数を量化してスキームを作る。
pub fn generalize(env: &TypeEnv, q: QualType) -> Scheme {
    let env_vars = env_ftv(env);
    let q_vars = ftv(&q.r#type);
    let vars: Vec<TVar> = q_vars
        .difference(&env_vars)
        .map(|id| TVar { id: *id })
        .collect();
    Scheme { vars, qual: q }
}

/// スキームの束縛変数を新しい型変数で置き換える。
pub fn instantiate(sc: &Scheme, supply: &mut TVarSupply) -> QualType {
    let mut sub: Subst = Subst::new();
    for tv in &sc.vars {
        sub.insert(tv.id, Type::TVar(supply.fresh()));
    }
    apply_subst_q(&sub, &sc.qual)
}

#[derive(Debug, Clone)]
/// 単一化が失敗したときの情報。
pub struct UnifyError {
    pub code: &'static str, // 例: TYPE001/TYPE002/TYPE090
    pub message: String,
}
impl UnifyError {
    /// エラーコードとメッセージを受け取るコンストラクタ。
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// 2つの型を突き合わせて最小の置換を得る。
pub fn unify(t1: Type, t2: Type) -> Result<Subst, UnifyError> {
    match (t1, t2) {
        (Type::TVar(tv), t) => bind(tv, t),
        (t, Type::TVar(tv)) => bind(tv, t),
        (Type::TCon(a), Type::TCon(b)) => {
            if a.name == b.name {
                Ok(Subst::new())
            } else {
                Err(UnifyError::new(
                    "TYPE001",
                    format!("型不一致: {:?} vs {:?}", a, b),
                ))
            }
        }
        (Type::TApp(a), Type::TApp(b)) => {
            let s1 = unify(*a.func.clone(), *b.func.clone())?;
            let s2 = unify(apply_subst_t(&s1, &a.arg), apply_subst_t(&s1, &b.arg))?;
            Ok(compose(&s2, &s1))
        }
        (Type::TFun(a), Type::TFun(b)) => {
            let s1 = unify(*a.arg.clone(), *b.arg.clone())?;
            let s2 = unify(apply_subst_t(&s1, &a.ret), apply_subst_t(&s1, &b.ret))?;
            Ok(compose(&s2, &s1))
        }
        (Type::TTuple(ta), Type::TTuple(tb)) => {
            if ta.items.len() != tb.items.len() {
                return Err(UnifyError::new("TYPE001", "タプル要素数が異なります"));
            }
            let mut s = Subst::new();
            for (a, b) in ta.items.into_iter().zip(tb.items.into_iter()) {
                let s_step = unify(apply_subst_t(&s, &a), apply_subst_t(&s, &b))?;
                s = compose(&s_step, &s);
            }
            Ok(s)
        }
        (x, y) => Err(UnifyError::new(
            "TYPE001",
            format!("型不一致: {:?} vs {:?}", x, y),
        )),
    }
}

/// 型変数と型を結び付けて置換とする。
pub fn bind(tv: TVar, t: Type) -> Result<Subst, UnifyError> {
    if let Type::TVar(TVar { id }) = &t {
        if *id == tv.id {
            return Ok(Subst::new());
        }
    }
    if ftv(&t).contains(&tv.id) {
        return Err(UnifyError::new("TYPE002", "オカーズチェック失敗"));
    }
    let mut s = Subst::new();
    s.insert(tv.id, t);
    Ok(s)
}

#[derive(Clone, Debug, Default)]
/// 型クラス階層とインスタンス集合を保持する。
pub struct ClassEnv {
    pub classes: HashMap<String, Vec<String>>, // クラス名 -> 上位クラス
    pub instances: HashSet<(String, String)>,  // (クラス名, 型コンストラクタ名)
}
impl ClassEnv {
    /// クラスと上位クラスの関係を登録する。
    pub fn add_class(
        &mut self,
        name: impl Into<String>,
        supers: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.classes
            .insert(name.into(), supers.into_iter().map(|s| s.into()).collect());
    }
    /// クラスに対するインスタンスを追加する。
    pub fn add_instance(&mut self, classname: impl Into<String>, tycon: impl Into<String>) {
        self.instances.insert((classname.into(), tycon.into()));
    }
    /// 複数の制約が満たされるかを判定する。
    pub fn entails(&self, cons: &[Constraint]) -> bool {
        cons.iter().all(|c| self.entails_one(c))
    }
    /// 単一の制約が満たされるかを判定する。
    fn entails_one(&self, c: &Constraint) -> bool {
        match &c.r#type {
            Type::TCon(tc) => self.has_instance(&c.classname, &tc.name),
            Type::TApp(TApp { func, arg }) => {
                if let Type::TCon(TCon { name }) = &**func {
                    if self.has_instance(&c.classname, name) {
                        return true;
                    }
                    if name == "[]" && (c.classname == "Eq" || c.classname == "Ord") {
                        return self.entails_one(&Constraint {
                            classname: c.classname.clone(),
                            r#type: (*arg.clone()),
                        });
                    }
                }
                false
            }
            Type::TTuple(tt) => {
                if c.classname == "Eq" || c.classname == "Ord" {
                    tt.items.iter().all(|t| {
                        self.entails_one(&Constraint {
                            classname: c.classname.clone(),
                            r#type: t.clone(),
                        })
                    })
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// インスタンス定義および上位クラスを再帰的に探索し、該当するものが存在するか調べる。
    fn has_instance(&self, cls: &str, tycon: &str) -> bool {
        if self
            .instances
            .contains(&(cls.to_string(), tycon.to_string()))
        {
            return true;
        }
        if let Some(supers) = self.classes.get(cls) {
            supers.iter().any(|s| self.has_instance(s, tycon))
        } else {
            false
        }
    }
}

/// 型をドキュメント向けに整形する補助関数（型変数に a, b, c… を割り当てる）。
fn pp_type(t: &Type, names: &mut HashMap<i64, String>) -> String {
    match t {
        Type::TVar(TVar { id }) => {
            if !names.contains_key(id) {
                let ch = (b'a' + (names.len() as u8)) as char;
                names.insert(*id, ch.to_string());
            }
            names
                .get(id)
                .cloned()
                .expect("型変数名の割当が存在する必要があります（pp_type 内部不変）")
        }
        Type::TCon(TCon { name }) => name.clone(),
        Type::TApp(TApp { func, arg }) => {
            if let Type::TCon(TCon { name }) = &**func {
                if name == "[]" {
                    return format!("[{}]", pp_type(arg, names));
                }
            }
            format!("{} {}", pp_type(func, names), pp_type(arg, names))
        }
        Type::TFun(TFun { arg, ret }) => {
            let mut a = pp_type(arg, names);
            if matches!(**arg, Type::TFun(_)) {
                a = format!("({})", a);
            }
            format!("{} -> {}", a, pp_type(ret, names))
        }
        Type::TTuple(TTuple { items }) => {
            if items.is_empty() {
                "()".to_string()
            } else {
                format!(
                    "({})",
                    items
                        .iter()
                        .map(|t| pp_type(t, names))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
    }
}

/// 型を内部で比較するためのキーに変換する。
fn key_of_type(t: &Type) -> String {
    match t {
        Type::TVar(TVar { id }) => format!("v{}", id),
        Type::TCon(TCon { name }) => format!("c{}", name),
        Type::TApp(TApp { func, arg }) => format!("a{}:{}", key_of_type(func), key_of_type(arg)),
        Type::TFun(TFun { arg, ret }) => format!("f{}:{}", key_of_type(arg), key_of_type(ret)),
        Type::TTuple(TTuple { items }) => {
            format!(
                "t({})",
                items.iter().map(key_of_type).collect::<Vec<_>>().join(",")
            )
        }
    }
}

/// 制約集合から重複を除いた新しいベクタを返す。
fn normalize_constraints(cs: &[Constraint]) -> Vec<Constraint> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for c in cs {
        let key = format!("{}|{}", c.classname, key_of_type(&c.r#type));
        if seen.insert(key) {
            out.push(c.clone());
        }
    }
    out
}

/// 型変数を含む制約のみを抽出する。
fn constraints_with_typevars(cs: &[Constraint]) -> Vec<Constraint> {
    cs.iter()
        .filter(|c| !ftv(&c.r#type).is_empty())
        .cloned()
        .collect()
}

/// 制約を `C a, D b => ` 形式に整形する。
fn pp_constraints(cs: &[Constraint], names: &mut HashMap<i64, String>) -> String {
    if cs.is_empty() {
        return String::new();
    }
    let mut cs2 = cs.to_vec();
    cs2.sort_by_key(|c| format!("{}|{}", c.classname, key_of_type(&c.r#type)));
    let mut parts = Vec::new();
    for c in cs2 {
        parts.push(format!("{} {}", c.classname, pp_type(&c.r#type, names)));
    }
    format!("{} => ", parts.join(", "))
}

/// 戻り値型に関与する制約だけを残す。
fn constraints_relevant_to_type(cs: &[Constraint], t: &Type) -> Vec<Constraint> {
    // 戻り値型に現れる型変数に関係する制約のみを表示対象にする。
    // これにより、戻り値が具体型（例: Double）の場合に曖昧制約を抑制できる。
    let tvs_t = ftv(t);
    cs.iter()
        .filter(|c| !ftv(&c.r#type).is_disjoint(&tvs_t))
        .cloned()
        .collect()
}

/// 制約つき型 `QualType` を人間に読みやすい文字列へ整形する。
///
/// - 不要な制約の抑制と安定した並び替えを行います。
///
/// # Examples
/// ```
/// use typelang::typesys::*;
/// let q = qualify(Type::TCon(TCon{ name: "Int".into() }), vec![]);
/// assert_eq!(pretty_qual(&q), "Int");
/// ```
pub fn pretty_qual(q: &QualType) -> String {
    let mut names: HashMap<i64, String> = HashMap::new();
    let cs = normalize_constraints(&q.constraints);
    let cs = constraints_with_typevars(&cs);
    let cs = constraints_relevant_to_type(&cs, &q.r#type);
    let mut s = String::new();
    s.push_str(&pp_constraints(&cs, &mut names));
    s.push_str(&pp_type(&q.r#type, &mut names));
    s
}

/// 曖昧な数値型変数を簡易に既定化（`Fractional -> Double`, `Num -> Integer`）。
/// 表示用のため、推論アルゴリズム自体の健全性には影響しません。
///
/// # Examples
/// ```
/// use typelang::typesys::*;
/// // Num a => a  を Integer に既定化
/// let q = QualType { constraints: vec![Constraint{ classname: "Num".into(), r#type: Type::TVar(TVar{ id: 0 }) }],
///                    r#type: Type::TVar(TVar{ id: 0 }) };
/// let d = apply_defaulting_simple(&q);
/// assert!(matches!(d.r#type, Type::TCon(TCon{ ref name }) if name == "Integer"));
/// ```
pub fn apply_defaulting_simple(q: &QualType) -> QualType {
    // 戻り値型を含め、曖昧さ解消のため積極的に既定化を試みる
    let mut sub: Subst = Subst::new();
    // まず Fractional 制約を優先して Double に写像する
    for c in &q.constraints {
        if c.classname == "Fractional" {
            if let Type::TVar(TVar { id }) = &c.r#type {
                sub.insert(
                    *id,
                    Type::TCon(TCon {
                        name: "Double".into(),
                    }),
                );
            }
        }
    }
    // 続いて、未設定の Num 制約を Integer に写像する
    for c in &q.constraints {
        if c.classname == "Num" {
            if let Type::TVar(TVar { id }) = &c.r#type {
                if !sub.contains_key(id) {
                    sub.insert(
                        *id,
                        Type::TCon(TCon {
                            name: "Integer".into(),
                        }),
                    );
                }
            }
        }
    }
    apply_subst_q(&sub, q)
}
