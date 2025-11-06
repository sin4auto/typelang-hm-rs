-- パス: examples/native_test.tl
-- ネイティブコンパイル確認用のシンプルなサンプル

intSquare :: Int -> Int;
let intSquare x = x * x;

main :: Int;
let main =
  let squareResult = intSquare 5 in
  squareResult;
