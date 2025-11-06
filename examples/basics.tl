-- パス: examples/basics.tl
-- 役割: Hindley–Milnerの基本構文サンプル
-- 意図: 型注釈とlet束縛の最小例を示す
-- 関連ファイル: examples/step1_numbers.tl, README.md

-- 型注釈 + let 束縛
id :: a -> a ;
let id x = x;

-- 制約付き多相
square :: Num a => a -> a ;
let square x = x * x;

-- 複数束縛（セミコロン区切り）
let const x y = x; let one = 1;

-- ネイティブビルド用のエントリポイント
main :: Int;
let main = square 3;
