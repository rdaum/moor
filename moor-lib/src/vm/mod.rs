pub mod execute;
pub mod opcode;

mod activation;

mod bf_list_sets;
mod bf_num;
mod bf_objects;
mod bf_server;
mod bf_strings;
mod bf_values;

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
                    frame: &mut Activation,
                    sess: Arc<RwLock<dyn Sessions>>,
                    args: Vec<Var>,
                ) -> Result<Var, anyhow::Error> {
                    $action(ws, frame, sess, args).await
                }
            }
        }
    };
}
