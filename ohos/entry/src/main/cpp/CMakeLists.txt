cmake_minimum_required(VERSION 3.5)
project(harmonydesk)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_FLAGS_DEBUG "-O3 -g")
set(CMAKE_CXX_FLAGS_RELEASE "-O3")

add_library(harmonydesk SHARED
    harmonydesk_napi.cpp
    video_decoder.cpp
)

target_compile_options(harmonydesk PRIVATE -O3)

target_link_libraries(harmonydesk PUBLIC
    libace_napi.z.so
    libhilog_ndk.z.so
    libnative_media_codecbase.so
    libnative_media_core.so
    libnative_media_vdec.so
    libnative_window.so
)
