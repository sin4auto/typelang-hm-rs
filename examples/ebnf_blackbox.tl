-- パス: examples/ebnf_blackbox.tl
-- 役割: TypeLang HM EBNF を網羅するブラックボックス仕様テスト
-- 意図: 字句・構文・型・評価の全要素を 1 ファイルで検証する
-- 関連ファイル: EBNF.md, src/parser.rs, tests/integration.rs, tests/lexer_parser.rs

-- 行コメントの検証

{- ブロックコメントのネスト検証
   {- 入れ子コメント -}
-}

data Maybe a = Nothing | Just a;
data Pair a b = Pair a b | Pair' (a, b);
data Tree a = Leaf a | Branch (Tree a) (Tree a) | Empty;
data Unit = Unit;

class Identity a;
class Phantom;
class Ranked a;

instance Identity Maybe;
instance Identity [];
instance Phantom Unit;
instance Ranked Tree;

maybeDefault :: a -> Maybe a -> a;
let maybeDefault fallback option =
  case option of
    Nothing -> fallback;
    chosen@(Just value) -> value

let patternJudge candidate =
  case candidate of
    Just 0 -> "zero";
    Just n | n < 0 -> "negative";
    Just n -> "positive";
    Nothing -> "missing"

arithSeries :: Integer -> (Integer, Integer, Integer, Integer);
let arithSeries n =
  let shifted = n + 9;
      euclidQ = div shifted 4;
      euclidR = mod shifted 4;
      truncQ = quot (n - 9) 4;
      truncR = rem (n - 9) 4
  in (euclidQ, euclidR, truncQ, truncR);

floatingCombo :: Fractional f => f -> f;
let floatingCombo x =
  let doubled = x * 2.0;
      halved = x / 2;
      root = x ** 0.5
  in (doubled - halved) + root;

structurePack :: a -> (a, [a], (a, a, a));
let structurePack value =
  let duplicate = [value, value];
      buildTriple center = (value, center, value)
  in (value, (duplicate :: [a]), buildTriple value);

applyTwice :: (a -> a) -> a -> a;
let applyTwice fn seed = (\g x -> g (g x)) fn seed;

let pipeline =
  let base = 3;
      tweaked = if base < 0 then base - 1 else base + 1;
      scaled y = y * 2 + 1;
      stepped = applyTwice scaled tweaked
  in stepped :: Int;

let nestedMatch tree =
  case tree of
    Leaf n -> n + 1;
    Branch left right ->
      let leftSum = nestedMatch left;
          rightSum = nestedMatch right
      in leftSum + rightSum;
    Empty -> 0;

let tupleInspector pair =
  case pair of
    Pair x y -> (x, y, True);
    alias@(Pair' (a, b)) -> (a, b, False);

let listSummary xs =
  case xs of
    [] -> "empty";
    [single] -> "singleton";
    pair@[_, _] -> "pair";
    trio@[_, _, _] -> "triple";
    _ -> "longer";

let compareTuple x y =
  ( x == y
  , x /= y
  , x < y
  , x <= y
  , x > y
  , x >= y
  );

orderingFacts :: (Eq a, Ord a) => a -> a -> (Bool, Bool, Bool, Bool, Bool, Bool);
let orderingFacts left right = compareTuple left right;

let scopedLet flag =
  let base = if flag then 1 else 0;
      bump x = x + 1;
      total = bump base
  in total :: Int;

let caseWithUnderscore value =
  case value of
    Just _ -> True;
    Nothing -> False;

let caseWithTuple tuple =
  case tuple of
    (a, b) -> a + b;

let matchUnit unit =
  case unit of
    () -> "unit";
    _ -> "not-unit";

let charClassifier ch =
  case ch of
    '\n' -> "newline";
    '\'' -> "quote";
    _ -> "other-char";

let stringClassifier text =
  case text of
    "hm" -> "grammar";
    _ -> "other-string";

let typedCase = (maybeDefault 42 (Just 0x2A)) :: Int;

let decimal = (42 :: Int);
let hexadecimal = (0x2A :: Int);
let octal = (0o77 :: Int);
let binary = (0b101010 :: Int);
negativeHex :: Int;
let negativeHex = -0xF;

let typedList = ([decimal, hexadecimal, octal, binary] :: [Int]);
let typedTuple = (decimal, hexadecimal, octal) :: (Int, Int, Int);
let typedMaybeList = (Just [decimal, hexadecimal] :: Maybe [Int]);

let boolList = [True, False];
let tupleOfBools = (True, False, True);
let unitValue = Unit;

let powerInt = 2 ^ 8;
let powerNegative = 2 ^ -1;
let powerFloat = 16.0 ** 0.25;

let scientificFloat = 1.5e3;
let rationalDivision = 5 / 2;

let escapedText = "Line1\nLine2\t\"quoted\"\\";
let newlineChar = '\n';
let quoteChar = '\'';
let backslashChar = '\\';

let pipelineNote = if True then "done" else "never";

let aliasBinding value =
  let same = value;
      another = same
  in another;

let letInTuple =
  let first = 1;
      second = 2
  in (first, second);

let counter' seed = seed + 1;

let holeValue placeholder = ?todo;

let typedLambda = (\n -> n + 1) (0 :: Int);

let annotateTuple = ((decimal, hexadecimal) :: (Int, Int));

let annotateListExpr = ([1, 2, 3] :: [Int]);

let stringList = ["alpha", "beta"];

let makeMaybePair x y = Just (Pair x y);

let nestedCase pairOpt =
  case pairOpt of
    Just (Pair x y) -> x + y;
    Nothing -> 0;

let guardExample n =
  case n of
    value | value >= 100 -> "large";
    _ -> "small";

let boolPipeline =
  let start = False;
      flipped = if start then True else False
  in flipped;

let tupleCase =
  case (decimal, hexadecimal, octal) of
    (a, b, c) -> a + b + c;

let listCase =
  case [decimal, hexadecimal, octal, binary] of
    [] -> 0;
    [only] -> only;
    prefix@[first, second] -> first + second;
    _ -> decimal;

let typedAnnotationInExpr =
  (applyTwice (\x -> x + 1) 5) :: Int;
