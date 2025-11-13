-- パス: examples/native_test.tl
-- ネイティブコンパイル確認用サンプル（println の挙動も検証）

intSquare :: Int -> Int;
let intSquare x = x * x;

verifyPrintln :: Int -> Int;
let verifyPrintln seed =
  let squared = intSquare seed;
      printedSquared = println squared;
      printedFlag = println (squared == 25)
  in squared;

main :: Int;
let main = verifyPrintln 5;
