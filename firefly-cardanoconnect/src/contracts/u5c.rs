use pallas_primitives::conway;
use pallas_traverse::ComputeHash as _;
use utxorpc_spec::utxorpc::v1alpha::cardano;

use crate::streams::BlockInfo;

fn convert_plutus_data(data: conway::PlutusData) -> cardano::PlutusData {
    let inner = match data {
        conway::PlutusData::Constr(con) => {
            let mut constr = cardano::Constr::default();
            constr.tag = con.tag as u32;
            constr.any_constructor = con.any_constructor.unwrap_or_default();
            for field in con.fields.to_vec() {
                constr.fields.push(convert_plutus_data(field));
            }
            cardano::plutus_data::PlutusData::Constr(constr)
        }
        conway::PlutusData::Map(map) => {
            let mut new_map = cardano::PlutusDataMap::default();
            for (key, value) in map.to_vec() {
                let mut pair = cardano::PlutusDataPair::default();
                pair.key = Some(convert_plutus_data(key));
                pair.value = Some(convert_plutus_data(value));
                new_map.pairs.push(pair);
            }
            cardano::plutus_data::PlutusData::Map(new_map)
        }
        conway::PlutusData::BigInt(int) => {
            let inner = match int {
                conway::BigInt::Int(i) => {
                    let value: i128 = i.into();
                    cardano::big_int::BigInt::Int(value as i64)
                }
                conway::BigInt::BigUInt(i) => cardano::big_int::BigInt::BigUInt(i.to_vec().into()),
                conway::BigInt::BigNInt(i) => cardano::big_int::BigInt::BigNInt(i.to_vec().into()),
            };
            let mut outer = cardano::BigInt::default();
            outer.big_int = Some(inner);
            cardano::plutus_data::PlutusData::BigInt(outer)
        }
        conway::PlutusData::BoundedBytes(bytes) => {
            cardano::plutus_data::PlutusData::BoundedBytes(bytes.to_vec().into())
        }
        conway::PlutusData::Array(array) => {
            let mut new_array = cardano::PlutusDataArray::default();
            for item in array.to_vec() {
                new_array.items.push(convert_plutus_data(item));
            }
            cardano::plutus_data::PlutusData::Array(new_array)
        }
    };
    let mut outer = cardano::PlutusData::default();
    outer.plutus_data = Some(inner);
    outer
}

fn convert_native_script(script: conway::NativeScript) -> cardano::NativeScript {
    fn convert_native_script_list(scripts: Vec<conway::NativeScript>) -> cardano::NativeScriptList {
        let mut new_list = cardano::NativeScriptList::default();
        for script in scripts {
            new_list.items.push(convert_native_script(script));
        }
        new_list
    }
    use cardano::native_script::NativeScript as InnerNativeScript;
    let inner = match script {
        conway::NativeScript::ScriptPubkey(hash) => {
            InnerNativeScript::ScriptPubkey(hash.to_vec().into())
        }
        conway::NativeScript::ScriptAll(scripts) => {
            InnerNativeScript::ScriptAll(convert_native_script_list(scripts))
        }
        conway::NativeScript::ScriptAny(scripts) => {
            InnerNativeScript::ScriptAny(convert_native_script_list(scripts))
        }
        conway::NativeScript::ScriptNOfK(k, scripts) => {
            let mut n_of_k = cardano::ScriptNOfK::default();
            n_of_k.k = k;
            for script in scripts {
                n_of_k.scripts.push(convert_native_script(script));
            }
            InnerNativeScript::ScriptNOfK(n_of_k)
        }
        conway::NativeScript::InvalidBefore(_) => todo!(),
        conway::NativeScript::InvalidHereafter(_) => todo!(),
    };
    let mut outer = cardano::NativeScript::default();
    outer.native_script = Some(inner);
    outer
}

fn convert_tx(bytes: &[u8]) -> cardano::Tx {
    use pallas_primitives::{alonzo, conway};

    let real_tx: conway::Tx = minicbor::decode(bytes).unwrap();
    let mut tx = cardano::Tx::default();
    for real_output in real_tx.transaction_body.outputs {
        let mut output = cardano::TxOutput::default();
        match real_output {
            conway::PseudoTransactionOutput::Legacy(txo) => {
                output.address = txo.address.to_vec().into();
                if let Some(hash) = txo.datum_hash {
                    let mut datum = cardano::Datum::default();
                    datum.hash = hash.to_vec().into();
                    output.datum = Some(datum);
                }
                match txo.amount {
                    alonzo::Value::Coin(c) => {
                        output.coin = c;
                    }
                    alonzo::Value::Multiasset(c, assets) => {
                        output.coin = c;
                        for (policy_id, policy_assets) in assets.iter() {
                            let assets = policy_assets
                                .iter()
                                .map(|(name, amount)| {
                                    let mut asset = cardano::Asset::default();
                                    asset.name = name.to_vec().into();
                                    asset.output_coin = *amount;
                                    asset
                                })
                                .collect();
                            let mut multiasset = cardano::Multiasset::default();
                            multiasset.policy_id = policy_id.to_vec().into();
                            multiasset.assets = assets;
                            output.assets.push(multiasset);
                        }
                    }
                }
            }
            pallas_primitives::conway::PseudoTransactionOutput::PostAlonzo(txo) => {
                output.address = txo.address.to_vec().into();
                if let Some(datum_option) = txo.datum_option {
                    let mut datum = cardano::Datum::default();
                    match datum_option {
                        conway::PseudoDatumOption::Hash(hash) => {
                            datum.hash = hash.to_vec().into();
                        }
                        conway::PseudoDatumOption::Data(data) => {
                            let mut cbor = vec![];
                            minicbor::encode(&data, &mut cbor).expect("infallible");
                            datum.hash = data.0.compute_hash().to_vec().into();
                            datum.payload = Some(convert_plutus_data(data.0));
                            datum.original_cbor = cbor.into();
                        }
                    }
                }
                match txo.value {
                    conway::Value::Coin(c) => {
                        output.coin = c;
                    }
                    conway::Value::Multiasset(c, assets) => {
                        output.coin = c;
                        for (policy_id, policy_assets) in assets.iter() {
                            let assets = policy_assets
                                .iter()
                                .map(|(name, amount)| {
                                    let mut asset = cardano::Asset::default();
                                    asset.name = name.to_vec().into();
                                    asset.output_coin = amount.into();
                                    asset
                                })
                                .collect();
                            let mut multiasset = cardano::Multiasset::default();
                            multiasset.policy_id = policy_id.to_vec().into();
                            multiasset.assets = assets;
                            output.assets.push(multiasset);
                        }
                    }
                }
                if let Some(script) = txo.script_ref {
                    let inner = match script.0 {
                        conway::PseudoScript::NativeScript(script) => {
                            cardano::script::Script::Native(convert_native_script(script))
                        }
                        conway::PseudoScript::PlutusV1Script(script) => {
                            cardano::script::Script::PlutusV1(script.0.to_vec().into())
                        }
                        conway::PseudoScript::PlutusV2Script(script) => {
                            cardano::script::Script::PlutusV2(script.0.to_vec().into())
                        }
                        conway::PseudoScript::PlutusV3Script(script) => {
                            cardano::script::Script::PlutusV3(script.0.to_vec().into())
                        }
                    };
                    let mut new_script = cardano::Script::default();
                    new_script.script = Some(inner);
                    output.script = Some(new_script);
                }
            }
        }
        tx.outputs.push(output);
    }
    tx
}

/**
 * Convert a block in our internal format (basically the bytes on the chain)
 * into one in balius format (mostly a list of utxorpc-formatted txos)
 */
pub fn convert_block(info: &BlockInfo) -> balius_runtime::Block {
    let header = cardano::BlockHeader {
        slot: info.block_slot.unwrap_or_default(),
        hash: hex::decode(&info.block_hash).unwrap().into(),
        height: info.block_height.unwrap_or_default(),
    };
    let body = cardano::BlockBody {
        tx: info.transactions.iter().map(|tx| convert_tx(tx)).collect(),
    };
    let block = cardano::Block {
        header: Some(header),
        body: Some(body),
    };
    balius_runtime::Block::Cardano(block)
}
