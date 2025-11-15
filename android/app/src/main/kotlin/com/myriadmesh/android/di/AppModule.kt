package com.myriadmesh.android.di

import android.content.Context
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import java.io.File
import javax.inject.Named
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object AppModule {

    @Provides
    @Singleton
    @Named("configDir")
    fun provideConfigDirectory(@ApplicationContext context: Context): File {
        return File(context.filesDir, "config").apply {
            if (!exists()) {
                mkdirs()
            }
        }
    }

    @Provides
    @Singleton
    @Named("dataDir")
    fun provideDataDirectory(@ApplicationContext context: Context): File {
        return File(context.filesDir, "data").apply {
            if (!exists()) {
                mkdirs()
            }
        }
    }

    @Provides
    @Singleton
    @Named("cacheDir")
    fun provideCacheDirectory(@ApplicationContext context: Context): File {
        return context.cacheDir
    }
}
