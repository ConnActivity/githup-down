FROM gitpod/workspace-mysql:latest

USER gitpod

# Install debugging for php and netcat
RUN sudo apt-get update -q \
    && sudo apt-get install php-xdebug php-dev automake autoconf netcat -y