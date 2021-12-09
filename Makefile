JEKYLL_VERSION:=3.9.0
IMAGE_NAME:=fsquirrel/ddanilov.me
CONTAINER_NAME:=ddanilov.me

.PHONY: help
help: ## Show this help
	@cat $(MAKEFILE_LIST) | grep -E '^[a-zA-Z_-]+:.*?## .*$$' | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

.PHONY: new
new: ## Generate new Jekyll site
	docker run -it --rm --name=${CONTAINER_NAME} \
		--volume="${PWD}:/srv/jekyll" \
		${IMAGE_NAME}:latest \
		jekyll new .

.PHONY: login
login: ## Login into Jekyll container
	docker run -it --rm --name=${CONTAINER_NAME} \
		--volume="${PWD}:/srv/jekyll" \
		${IMAGE_NAME}:latest \
		bash;exit

.PHONY: run
run: ## Run Jekyll site
	docker run -it --rm --name=${CONTAINER_NAME} \
		--volume="${PWD}:/srv/jekyll" \
		-p 4000:4000 \
		${IMAGE_NAME}:latest \
		bundle exec jekyll serve -H 0.0.0.0

.PHONY: rund
rund: ## Run Jekyll site in background
	docker run -it --rm --detach --name=${CONTAINER_NAME} \
		--volume="${PWD}:/srv/jekyll" \
		-p 4000:4000 \
		${IMAGE_NAME}:latest \
		bundle exec jekyll serve -H 0.0.0.0

.PHONY: stop
stop: ## Stop Jekyll site
	docker stop ${CONTAINER_NAME}

.PHONY: bundle
bundle: ## Install dependencies
	docker run -it --rm --name=${CONTAINER_NAME} \
		--volume="${PWD}:/srv/jekyll" \
		${IMAGE_NAME}:latest \
		bundle install

.PHONY: build-image
build-image: ## Build image
	docker build -t ${IMAGE_NAME} .
