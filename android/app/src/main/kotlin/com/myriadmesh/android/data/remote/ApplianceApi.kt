package com.myriadmesh.android.data.remote

import com.myriadmesh.android.data.remote.dto.*
import retrofit2.Response
import retrofit2.http.*

/**
 * Retrofit API interface for MyriadNode Appliance REST API.
 * Based on the Appliance API specification.
 */
interface ApplianceApi {

    // ===== Information & Status =====

    @GET("api/appliance/info")
    suspend fun getApplianceInfo(): Response<ApplianceInfoDto>

    @GET("api/appliance/stats")
    suspend fun getApplianceStats(): Response<ApplianceStatsDto>

    // ===== Pairing =====

    @POST("api/appliance/pair/request")
    suspend fun requestPairing(
        @Body request: PairingRequestDto
    ): Response<PairingResponseDto>

    @POST("api/appliance/pair/approve/{token}")
    suspend fun approvePairing(
        @Path("token") token: String
    ): Response<Unit>

    @POST("api/appliance/pair/reject/{token}")
    suspend fun rejectPairing(
        @Path("token") token: String
    ): Response<Unit>

    @POST("api/appliance/pair/complete")
    suspend fun completePairing(
        @Body request: CompletePairingDto
    ): Response<CompletePairingResponseDto>

    // ===== Device Management =====

    @GET("api/appliance/devices")
    suspend fun getDevices(
        @Header("X-Session-Token") sessionToken: String
    ): Response<DevicesResponseDto>

    @GET("api/appliance/devices/{device_id}")
    suspend fun getDevice(
        @Header("X-Session-Token") sessionToken: String,
        @Path("device_id") deviceId: String
    ): Response<PairedDeviceDto>

    @POST("api/appliance/devices/{device_id}/unpair")
    suspend fun unpairDevice(
        @Header("X-Session-Token") sessionToken: String,
        @Path("device_id") deviceId: String
    ): Response<Unit>

    @POST("api/appliance/devices/{device_id}/preferences")
    suspend fun updatePreferences(
        @Header("X-Session-Token") sessionToken: String,
        @Path("device_id") deviceId: String,
        @Body preferences: DevicePreferencesDto
    ): Response<Unit>

    // ===== Message Caching =====

    @POST("api/appliance/cache/store")
    suspend fun storeMessage(
        @Header("X-Session-Token") sessionToken: String,
        @Body request: StoreCachedMessageDto
    ): Response<Unit>

    @GET("api/appliance/cache/retrieve")
    suspend fun retrieveMessages(
        @Header("X-Session-Token") sessionToken: String,
        @Query("device_id") deviceId: String,
        @Query("limit") limit: Int? = null,
        @Query("priority") priority: Int? = null
    ): Response<RetrieveCachedMessagesDto>

    @POST("api/appliance/cache/delivered")
    suspend fun markMessagesDelivered(
        @Header("X-Session-Token") sessionToken: String,
        @Body request: MarkDeliveredDto
    ): Response<Unit>

    @GET("api/appliance/cache/stats/{device_id}")
    suspend fun getCacheStats(
        @Header("X-Session-Token") sessionToken: String,
        @Path("device_id") deviceId: String
    ): Response<CacheStatsDto>
}
