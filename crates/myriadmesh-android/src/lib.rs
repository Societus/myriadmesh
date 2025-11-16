use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

mod node;
use node::AndroidNode;

// SECURITY C10: Global handle registry for safe JNI pointer management
// Prevents use-after-free, double-free, and memory leaks
static ANDROID_NODES: Lazy<Mutex<HashMap<u64, Arc<Mutex<AndroidNode>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

/// Initialize the MyriadNode for Android.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
/// Uses handle registry to prevent memory leaks and use-after-free bugs.
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeInit(
    mut env: JNIEnv,
    _class: JClass,
    config_path: JString,
    data_dir: JString,
) -> jlong {
    // Initialize Android logger
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("MyriadMesh"),
    );

    log::info!("Initializing MyriadNode for Android");

    // Convert JString to Rust String
    let config_path: String = match env.get_string(&config_path) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get config path: {:?}", e);
            return 0;
        }
    };

    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get data dir: {:?}", e);
            return 0;
        }
    };

    log::debug!("Config path: {}", config_path);
    log::debug!("Data dir: {}", data_dir);

    // Create the node
    match AndroidNode::new(config_path, data_dir) {
        Ok(node) => {
            // SECURITY C10: Use handle registry instead of raw pointers
            let handle = NEXT_HANDLE.fetch_add(1, Ordering::SeqCst);

            match ANDROID_NODES.lock() {
                Ok(mut nodes) => {
                    nodes.insert(handle, Arc::new(Mutex::new(node)));
                    log::info!("MyriadNode initialized successfully (handle: {})", handle);
                    handle as jlong
                }
                Err(e) => {
                    log::error!("Failed to acquire lock on node registry: {:?}", e);
                    0
                }
            }
        }
        Err(e) => {
            log::error!("Failed to initialize node: {:?}", e);
            0
        }
    }
}

/// Start the MyriadNode.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
/// SECURITY C10: Uses handle validation to prevent use-after-free.
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeStart(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jboolean {
    if handle == 0 {
        log::error!("Invalid handle (0)");
        return JNI_FALSE;
    }

    // SECURITY C10: Validate handle and get node from registry
    let nodes = match ANDROID_NODES.lock() {
        Ok(nodes) => nodes,
        Err(e) => {
            log::error!("Failed to acquire lock on node registry: {:?}", e);
            return JNI_FALSE;
        }
    };

    match nodes.get(&(handle as u64)) {
        Some(node_arc) => {
            let mut node = match node_arc.lock() {
                Ok(node) => node,
                Err(e) => {
                    log::error!("Failed to lock node: {:?}", e);
                    return JNI_FALSE;
                }
            };

            match node.start() {
                Ok(_) => {
                    log::info!("MyriadNode started successfully");
                    JNI_TRUE
                }
                Err(e) => {
                    log::error!("Failed to start node: {:?}", e);
                    JNI_FALSE
                }
            }
        }
        None => {
            log::error!("Invalid handle: {}", handle);
            JNI_FALSE
        }
    }
}

/// Stop the MyriadNode.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
/// SECURITY C10: Uses handle validation to prevent use-after-free.
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeStop(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jboolean {
    if handle == 0 {
        log::error!("Invalid handle (0)");
        return JNI_FALSE;
    }

    let nodes = match ANDROID_NODES.lock() {
        Ok(nodes) => nodes,
        Err(e) => {
            log::error!("Failed to acquire lock on node registry: {:?}", e);
            return JNI_FALSE;
        }
    };

    match nodes.get(&(handle as u64)) {
        Some(node_arc) => {
            let mut node = match node_arc.lock() {
                Ok(node) => node,
                Err(e) => {
                    log::error!("Failed to lock node: {:?}", e);
                    return JNI_FALSE;
                }
            };

            match node.stop() {
                Ok(_) => {
                    log::info!("MyriadNode stopped successfully");
                    JNI_TRUE
                }
                Err(e) => {
                    log::error!("Failed to stop node: {:?}", e);
                    JNI_FALSE
                }
            }
        }
        None => {
            log::error!("Invalid handle: {}", handle);
            JNI_FALSE
        }
    }
}

/// Send a message through the mesh network.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
/// SECURITY C10: Uses handle validation to prevent use-after-free.
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeSendMessage(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    destination: JString,
    payload: JByteArray,
    priority: jint,
) -> jboolean {
    if handle == 0 {
        log::error!("Invalid handle (0)");
        return JNI_FALSE;
    }

    // Convert destination
    let destination: String = match env.get_string(&destination) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get destination: {:?}", e);
            return JNI_FALSE;
        }
    };

    // Convert payload
    let payload_bytes = match env.convert_byte_array(&payload) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("Failed to convert payload: {:?}", e);
            return JNI_FALSE;
        }
    };

    let nodes = match ANDROID_NODES.lock() {
        Ok(nodes) => nodes,
        Err(e) => {
            log::error!("Failed to acquire lock on node registry: {:?}", e);
            return JNI_FALSE;
        }
    };

    match nodes.get(&(handle as u64)) {
        Some(node_arc) => {
            let node = match node_arc.lock() {
                Ok(node) => node,
                Err(e) => {
                    log::error!("Failed to lock node: {:?}", e);
                    return JNI_FALSE;
                }
            };

            match node.send_message(&destination, &payload_bytes, priority as u8) {
                Ok(_) => {
                    log::debug!("Message queued for delivery to {}", destination);
                    JNI_TRUE
                }
                Err(e) => {
                    log::error!("Failed to send message: {:?}", e);
                    JNI_FALSE
                }
            }
        }
        None => {
            log::error!("Invalid handle: {}", handle);
            JNI_FALSE
        }
    }
}

/// Get the node's public ID.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
/// SECURITY C10: Uses handle validation to prevent use-after-free.
#[no_mangle]
#[allow(unused_mut)] // env.new_string() requires mutable borrow
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeGetNodeId(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    if handle == 0 {
        log::error!("Invalid handle (0)");
        return std::ptr::null_mut();
    }

    let nodes = match ANDROID_NODES.lock() {
        Ok(nodes) => nodes,
        Err(e) => {
            log::error!("Failed to acquire lock on node registry: {:?}", e);
            return std::ptr::null_mut();
        }
    };

    match nodes.get(&(handle as u64)) {
        Some(node_arc) => {
            let node = match node_arc.lock() {
                Ok(node) => node,
                Err(e) => {
                    log::error!("Failed to lock node: {:?}", e);
                    return std::ptr::null_mut();
                }
            };

            match node.get_node_id() {
                Ok(node_id) => match env.new_string(&node_id) {
                    Ok(s) => s.into_raw(),
                    Err(e) => {
                        log::error!("Failed to create JString: {:?}", e);
                        std::ptr::null_mut()
                    }
                },
                Err(e) => {
                    log::error!("Failed to get node ID: {:?}", e);
                    std::ptr::null_mut()
                }
            }
        }
        None => {
            log::error!("Invalid handle: {}", handle);
            std::ptr::null_mut()
        }
    }
}

/// Get the node's status as JSON.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
/// SECURITY C10: Uses handle validation to prevent use-after-free.
#[no_mangle]
#[allow(unused_mut)] // env.new_string() requires mutable borrow
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeGetStatus(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    if handle == 0 {
        log::error!("Invalid handle (0)");
        return std::ptr::null_mut();
    }

    let nodes = match ANDROID_NODES.lock() {
        Ok(nodes) => nodes,
        Err(e) => {
            log::error!("Failed to acquire lock on node registry: {:?}", e);
            return std::ptr::null_mut();
        }
    };

    match nodes.get(&(handle as u64)) {
        Some(node_arc) => {
            let node = match node_arc.lock() {
                Ok(node) => node,
                Err(e) => {
                    log::error!("Failed to lock node: {:?}", e);
                    return std::ptr::null_mut();
                }
            };

            match node.get_status() {
                Ok(status) => match env.new_string(&status) {
                    Ok(s) => s.into_raw(),
                    Err(e) => {
                        log::error!("Failed to create JString: {:?}", e);
                        std::ptr::null_mut()
                    }
                },
                Err(e) => {
                    log::error!("Failed to get status: {:?}", e);
                    std::ptr::null_mut()
                }
            }
        }
        None => {
            log::error!("Invalid handle: {}", handle);
            std::ptr::null_mut()
        }
    }
}

/// Destroy the node and free resources.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
/// SECURITY C10: Removes node from handle registry, Arc cleanup is automatic.
/// Prevents double-free and memory leaks.
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle == 0 {
        log::warn!("Attempted to destroy invalid handle (0)");
        return;
    }

    // SECURITY C10: Remove from registry, Arc will drop when last reference is gone
    match ANDROID_NODES.lock() {
        Ok(mut nodes) => {
            if nodes.remove(&(handle as u64)).is_some() {
                log::info!("MyriadNode destroyed (handle: {})", handle);
            } else {
                log::warn!("Attempted to destroy non-existent handle: {}", handle);
            }
        }
        Err(e) => {
            log::error!("Failed to acquire lock on node registry: {:?}", e);
        }
    }
}
