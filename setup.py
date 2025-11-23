from setuptools import setup, Extension
import pybind11
import subprocess
import sys

# Get OpenCV paths using pkg-config
def get_opencv_flags():
    try:
        cflags = subprocess.check_output(['pkg-config', '--cflags', 'opencv4']).decode().strip().split()
        libs = subprocess.check_output(['pkg-config', '--libs', 'opencv4']).decode().strip().split()
        return cflags, libs
    except:
        # Fallback for homebrew on macOS
        try:
            cflags = subprocess.check_output(['pkg-config', '--cflags', 'opencv']).decode().strip().split()
            libs = subprocess.check_output(['pkg-config', '--libs', 'opencv']).decode().strip().split()
            return cflags, libs
        except:
            print("Warning: pkg-config not found, using default paths")
            # Common macOS homebrew paths
            return (
                ['-I/opt/homebrew/include/opencv4', '-I/usr/local/include/opencv4'],
                ['-L/opt/homebrew/lib', '-L/usr/local/lib', '-lopencv_core', '-lopencv_imgproc']
            )

cflags, libs = get_opencv_flags()

# Parse flags
include_dirs = [pybind11.get_include()]
library_dirs = []
libraries = []
extra_compile_args = ['-std=c++14', '-O3']
extra_link_args = []

for flag in cflags:
    if flag.startswith('-I'):
        include_dirs.append(flag[2:])
    else:
        extra_compile_args.append(flag)

for flag in libs:
    if flag.startswith('-L'):
        library_dirs.append(flag[2:])
    elif flag.startswith('-l'):
        libraries.append(flag[2:])
    else:
        extra_link_args.append(flag)

ext_modules = [
    Extension(
        'fast_processor',
        ['backend/core/fast_processor.cpp'],
        include_dirs=include_dirs,
        library_dirs=library_dirs,
        libraries=libraries,
        extra_compile_args=extra_compile_args,
        extra_link_args=extra_link_args,
        language='c++'
    ),
]

setup(
    name='fast_processor',
    version='0.1.0',
    author='Your Name',
    description='Fast C++ video processing extension',
    ext_modules=ext_modules,
    install_requires=['pybind11'],
    zip_safe=False,
)
