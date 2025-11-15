use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_TRUE, JNI_FALSE};
use jni::JNIEnv;
use std::sync::Arc;
use tokio::runtime::Runtime;

mod node;
use node::AndroidNode;

/// Initialize the MyriadNode for Android.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
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
            let node_ptr = Box::into_raw(Box::new(node));
            log::info!("MyriadNode initialized successfully");
            node_ptr as jlong
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
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeStart(
    _env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
) -> jboolean {
    if node_ptr == 0 {
        log::error!("Null node pointer");
        return JNI_FALSE;
    }

    let node = &mut *(node_ptr as *mut AndroidNode);

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

/// Stop the MyriadNode.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeStop(
    _env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
) -> jboolean {
    if node_ptr == 0 {
        log::error!("Null node pointer");
        return JNI_FALSE;
    }

    let node = &mut *(node_ptr as *mut AndroidNode);

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

/// Send a message through the mesh network.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeSendMessage(
    mut env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
    destination: JString,
    payload: JByteArray,
    priority: jint,
) -> jboolean {
    if node_ptr == 0 {
        log::error!("Null node pointer");
        return JNI_FALSE;
    }

    let node = &mut *(node_ptr as *mut AndroidNode);

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

/// Get the node's public ID.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeGetNodeId(
    mut env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
) -> jstring {
    if node_ptr == 0 {
        log::error!("Null node pointer");
        return std::ptr::null_mut();
    }

    let node = &*(node_ptr as *const AndroidNode);

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

/// Get the node's status as JSON.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeGetStatus(
    mut env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
) -> jstring {
    if node_ptr == 0 {
        log::error!("Null node pointer");
        return std::ptr::null_mut();
    }

    let node = &*(node_ptr as *const AndroidNode);

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

/// Destroy the node and free resources.
///
/// # Safety
/// This function is called from JNI and must handle all errors safely.
#[no_mangle]
pub unsafe extern "C" fn Java_com_myriadmesh_android_core_MyriadNode_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    node_ptr: jlong,
) {
    if node_ptr == 0 {
        log::warn!("Attempted to destroy null node pointer");
        return;
    }

    let node = Box::from_raw(node_ptr as *mut AndroidNode);
    drop(node);
    log::info!("MyriadNode destroyed");
}
