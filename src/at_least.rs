use async_std::future::Future;

pub trait AtLeastN<I>
where
    I: Iterator,
{
    fn at_least<P>(self, n: usize, predicate: P) -> bool
    where
        P: FnMut(&I::Item) -> bool;
}

impl<I: Iterator> AtLeastN<I> for I {
    fn at_least<P>(self, n: usize, predicate: P) -> bool
    where
        P: FnMut(&I::Item) -> bool,
    {
        self.filter(predicate).take(n).count() == n
    }
}

#[async_trait::async_trait]
pub trait AtLeastNAsync<I>
where
    I: Iterator,
{
    async fn at_least_async<P, F>(&mut self, n: usize, predicate: P) -> bool
    where
        P: Send + FnMut(I::Item) -> F,
        F: Send + Future<Output = bool>;
}

#[async_trait::async_trait]
impl<I> AtLeastNAsync<I> for I
where
    I: Iterator<Item: Send> + Send,
{
    async fn at_least_async<P, F>(&mut self, n: usize, mut predicate: P) -> bool
    where
        P: Send + FnMut(I::Item) -> F,
        F: Send + Future<Output = bool>,
    {
        if n == 0 {
            return true;
        }

        let mut count = 0;
        for item in self { // not concurrent yet
            if predicate(item).await {
                count += 1;
            }

            if count >= n {
                return true;
            }
        }

        false
    }
}
