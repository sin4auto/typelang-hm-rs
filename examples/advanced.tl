-- 制約付き・注釈付き・負の指数・タプルの簡単な例

-- 制約付き（多相）
plus1 :: Num a => a -> a ;
let plus1 x = x + 1;

-- 注釈付き
let one_int = (1 :: Int);

-- 負の指数（Double にフォールバック）
let inv2 = 2 ^ -1;
let powf = 2 ** -1;

-- タプル（型表示は簡略化される）
let make_pair x y = (x, y);

