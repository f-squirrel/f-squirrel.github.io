JEKYLL_VERSION:=3.9.0
IMAGE_NAME:=ddanilov.me
CONTAINER_NAME:=${IMAGE_NAME}

.PHONY: help
help: ## The Makefile helps to build Concord-BFT in a docker container
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

.PHONY: bundle
bundle: ## Install dependencies
	docker run -it --rm --name=${CONTAINER_NAME} \
		--volume="${PWD}:/srv/jekyll" \
		${IMAGE_NAME}:latest \
		bundle install

.PHONY: build-image
build-image: ## Build image
	docker build -t ${IMAGE_NAME} .
