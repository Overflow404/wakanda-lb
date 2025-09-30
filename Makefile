.PHONY: all down build up

all: down build up

down:
	docker-compose down

build:
	docker-compose build

up:
	docker-compose up -d
