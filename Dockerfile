# Use an official Python runtime as a parent image
FROM python:3.9-slim

# Set the working directory in the container to /app
WORKDIR /app

# Copy the current directory contents into the container at /app
COPY . .

# Install any needed packages specified in requirements.txt
RUN pip install -r requirements.txt

# Install graphics package
RUN apt-get update && apt-get install -y \
    libgl1-mesa-glx \
    libglib2.0-0 \
&& rm -rf /var/lib/apt/lists/*

# Create a user and switch to it
RUN adduser --disabled-password --gecos '' abbeyroad
USER abbeyroad
