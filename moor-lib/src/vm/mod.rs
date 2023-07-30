pub mod opcode;
pub mod vm_execute;
pub mod vm_unwind;

mod activation;

mod bf_list_sets;
mod bf_num;
mod bf_objects;
mod bf_server;
mod bf_strings;
mod bf_values;
pub(crate) mod vm;

mod bf_verbs;
#[cfg(test)]
mod vm_test;

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
                async fn call<'a>(
                    &self,
                    bf_args: &mut BfFunctionArguments<'a>
                ) -> Result<Var, anyhow::Error> {
                    $action(bf_args).await
                }
            }
        }
    };
}
