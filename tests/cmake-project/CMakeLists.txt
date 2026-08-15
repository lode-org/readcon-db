cmake_minimum_required(VERSION 3.22)
project(readcon-db-cmake-test C)

option(READCON_DB_USE_FETCHCONTENT "Exercise FetchContent_Declare(SOURCE_DIR=...)" ON)

if(DEFINED READCON_DB_SOURCE_DIR)
    if(READCON_DB_USE_FETCHCONTENT)
        include(FetchContent)
        FetchContent_Declare(readcon-db SOURCE_DIR "${READCON_DB_SOURCE_DIR}")
        FetchContent_MakeAvailable(readcon-db)
    else()
        add_subdirectory("${READCON_DB_SOURCE_DIR}" "${CMAKE_BINARY_DIR}/readcon-db")
    endif()
else()
    find_package(readcon-db REQUIRED CONFIG)
endif()

add_executable(c-main main.c)
target_link_libraries(c-main PRIVATE readcon-db::shared)

add_executable(c-main-static main.c)
target_link_libraries(c-main-static PRIVATE readcon-db::static)
