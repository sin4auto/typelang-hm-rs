-- パス: examples/ebnf_blackbox.tl
-- 役割: TypeLang HM EBNF を網羅するブラックボックス仕様テスト
-- 意図: 字句・構文・式・型の全要素を 1 ファイルで検証する
-- 関連ファイル: EBNF.md, src/parser.rs, tests/integration.rs

{- ブロックコメントのネスト確認
   {- 入れ子も許容される -}
-}

data Maybe a = Nothing | Just a;
data Color = Red | Green | Blue;
data Box a = Box a;

arithSeries :: Integer -> (Integer, Integer, Integer, Integer);
let arithSeries n =
  let euclidQ = div (n + 9) 4;
      euclidR = mod (n + 9) 4;
      truncQ = quot (n - 9) 4;
      truncR = rem (n - 9) 4
  in (euclidQ, euclidR, truncQ, truncR);

floatingCombo :: Fractional f => f -> f;
let floatingCombo x = (x / 2) + (x ** 0.5);

structurePack :: a -> (a, [a], (a, a, a));
let structurePack value =
  let duplicate = [value, value];
      embed y = (value, y, value)
  in (value, (duplicate :: [a]), embed value);

applyTwice :: (a -> a) -> a -> a;
let applyTwice fn seed = (\g x -> g (g x)) fn seed;

let colorFlag shade =
  case shade of
    Red -> 1;
    Green -> 2;
    Blue -> 3;
    _ -> 0;

let classifyNumber n =
  if n < 0 then "negative"
  else if n == 0 then "zero"
  else "positive";

let shiftNum n = ((n + 1) - 2) * 3 / 3;

let patternJudge candidate =
  case candidate of
    Just 0 -> "zero";
    Just n -> classifyNumber n;
    Nothing -> "missing"

maybeDefault :: a -> Maybe a -> a;
let maybeDefault fallback option =
  case option of
    Nothing -> fallback;
    Just value -> value;

let boxUnwrap box =
  case box of
    Box value -> value;

let escapedText = "Line1\nLine2\t\"quoted\"\\";
let newlineChar = '\n';
let quoteChar = '\'';

let decimal = (42 :: Int);
let hexadecimal = (0x2A :: Int);
let octal = (0o77 :: Int);
let binary = (0b101010 :: Int);
negativeHex :: Int;
let negativeHex = -0xF;

let typedList = ([decimal, hexadecimal, octal, binary] :: [Int]);
let typedTuple = ((colorFlag Red, colorFlag Green, colorFlag Blue) :: (Int, Int, Int));

let counter' start = start + 1;

let pipeline =
  applyTwice (\value -> let scaled = value * 2 in scaled + 1) 3;

let scopedComputation flag =
  let base = if flag then (1 :: Int) else (0 :: Int);
      shift m = m + 1;
      adjust v = v + shift base
  in (adjust base) :: Int;

let typedCase = (maybeDefault decimal (Just decimal)) :: Int;

let boxSample = Box decimal;
let boxValue = boxUnwrap boxSample;

let maybeBox = Just (Box decimal);
let nestedCase =
  case maybeBox of
    Just (Box x) -> x;
    Nothing -> decimal

let boolList = [True, False];

let powerInt = 2 ^ 8;
let powerNegative = 2 ^ -1;
let powerFloat = 16 ** 0.5;

let debugString = "?todo";
