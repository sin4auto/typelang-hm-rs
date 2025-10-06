-- パス: examples/ebnf_blackbox.tl
-- 役割: TypeLang HM EBNF を網羅するブラックボックステスト
-- 意図: 字句・型・式の仕様を 1 ファイルで検証する
-- 関連ファイル: EBNF.md, src/parser.rs, examples/advanced.tl

{- ブロックコメントのネスト確認
   {- 入れ子も許容される -}
-}

-- 多重制約と比較演算子の網羅
compareTuple :: (Eq a, Ord a) => a -> a -> (Bool, Bool, Bool, Bool, Bool, Bool);
let compareTuple left right =
  ( left == right
  , left /= right
  , left < right
  , left <= right
  , left > right
  , left >= right
  );

-- 単一制約と数値演算の網羅
shiftNum :: Num n => n -> n;
let shiftNum n = ((n + 1) - 2) * 3 / 3;

-- let-in と複数 bind、タプル・リスト型注釈を含む例
collector :: a -> (a, [a], (a, a, a));
let collector item =
  let repeated = [item, item];
      triplet x = (item, x, item)
  in (item, (repeated :: [a]), triplet item);

-- λ式と関数適用の確認
applyTwice :: (a -> a) -> a -> a;
let applyTwice fn seed = (\g x -> g (g x)) fn seed;

-- 未使用引数と varid に `'` を含む例
let dropFirst unused rest = rest;
let counter' start = start + 1;

let decimal = (42 :: Int);
let hexadecimal = (0x2A :: Int);
let octal = (0o77 :: Int);
let binary = (0b101010 :: Int);
let negativeHex = (-0xF :: Int);

-- 浮動小数点リテラルと指数表記
let floatPoint = 3.1415;
let floatExponent = 6.02e23;
let floatMixed = 1.0e-2;

-- 文字・文字列リテラルとエスケープ
let newlineChar = '\n';
let quoteChar = '\'';
let escapedString = "Line1\nLine2\t\"quoted\"\\";

-- True / False と if-then-else の網羅
let classifyNumber n =
  if n < 0 then "negative"
  else if n == 0 then "zero"
  else "positive";

-- let-in の追加例と型注釈付き式
let scopedComputation flag =
  let base = if flag then (1 :: Int) else (0 :: Int);
      adjust v = v + shiftNum base
  in (adjust base) :: Int;

-- リスト・タプル・型注釈の網羅
let typedList = ([decimal, hexadecimal, octal, binary] :: [Int]);
let typedTuple = ((compareTuple True False) :: (Bool, Bool, Bool, Bool, Bool, Bool));

-- 型注釈を伴うシンプルな束縛
phantomMaybe :: Int;
let phantomMaybe = decimal;

-- 冪乗演算子と連鎖適用
let powerInt = 2 ^ 8;
let powerNegative = 2 ^ -1;
let powerFloat = 16 ** 0.5;

-- アプリケーション連鎖の例
let pipeline = applyTwice (
  \value ->
    let scaled = value * 2
    in scaled + 1
  ) 3;

let sequential = shiftNum 10; let annotated = (collector True);

-- '?' プレースホルダ文字列の例
let debugHole = "?todo";
