-- 総合的に構文と型クラスをカバーするサンプルプログラム

-- 基本多相関数とリテラル
let id x = x;
let flag = True;
let greeting = "Hello, TypeLang!";
let firstLetter = 'T';
let numbers = [1, 2, 3, 4];
let sampleTuple = (42, "answer", False);

-- 型クラス制約が推論される関数
let square x = x * x;
let generalAbs x = if x < 0 then -x else x;
let numericId x = x;

-- 演算子の挙動
let powIntExample = 2 ^ 8;
let powFracExample = 2.0 ** 0.5;

-- let-in と複数バインディング
let twoSquares a b =
  let aa = a ^ 2;
      bb = b ^ 2
  in aa + bb;

-- ラムダ式と関数合成
let compose f g = \x -> f (g x);
let applyTwice f = \x -> f (f x);

-- 条件分岐のバリエーション
let equalOrNot x y = if x == y then True else False;
let choiceMessage = if flag then greeting else "fallback";

-- Double 計算例
let combinedDemo =
  let s = square 3.0;
      p = powFracExample;
      q = 2.0 ** 3.0
  in s + p + q;

-- 型注釈とクラス制約の動作確認
let annotatedSquare = (square 5 :: Int);

let numericIdInt = (numericId 7 :: Int);

let equalityCheck = sampleTuple == (42, "answer", False);
let orderingCheck = numbers < [1, 2, 3, 5];
let tupleOrderingCheck = (1, 2, 3) > (1, 2, 1);
let charOrdering = 'A' < firstLetter;
let shownFlag = show flag;
let shownGreeting = show greeting;
let fractionalDemo = 7 / 2;

-- Functor / Foldable の利用例
let mappedNumbers = map (\x -> x + 10) numbers;

let foldedSum = foldl (\acc x -> acc + x) 0 numbers;
let foldedProduct = foldr (\x acc -> x * acc) 1 [2, 3, 4];

-- 高階関数の結果検証
let composeResult = (compose square (\x -> x + 1)) 3;
let applyTwiceResult = (applyTwice (\x -> x - 1)) 5;
