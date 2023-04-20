pub mod execute;
pub mod opcode;

mod activation;

mod bf_values;
mod bf_num;
mod bf_server;

#[macro_export]
macro_rules! bf_declare {
    ( $name:ident, $action:expr ) => {
        paste::item! {
            pub struct [<Bf $name:camel >] {}
            #[async_trait]
            impl BfFunction for [<Bf $name:camel >] {
                fn name(&self) -> &str {
                    return stringify!($name)
                }
                async fn call(
                    &self,
                    ws: &mut dyn WorldState,
                    sess: Arc<Mutex<dyn Sessions>>,
                    args: Vec<Var>,
                ) -> Result<Var, anyhow::Error> {
                    $action(ws, sess, args).await
                }
            }
        }
    };
}
