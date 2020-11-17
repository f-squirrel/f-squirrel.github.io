JEKYLL_VERSION:=3.9.0
IMAGE_NAME:=ddanilov1.me

.PHONY: new
new:
	docker run -it --rm --name=ddanilov1.me \
		--volume="${PWD}:/srv/jekyll" \
		${IMAGE_NAME}:latest \
		jekyll new .

.PHONY: login
login:
	docker run -it --rm --name=ddanilov1.me \
		--volume="${PWD}:/srv/jekyll" \
		${IMAGE_NAME}:latest \
		bash;exit

.PHONY: run
run:
	docker run -it --rm --name=ddanilov1.me \
		--volume="${PWD}:/srv/jekyll" \
		-p 4000:4000 \
		${IMAGE_NAME}:latest \
		bundle exec jekyll serve -H 0.0.0.0

.PHONY:bundle
bundle:
	docker run -it --rm --name=ddanilov1.me \
		--volume="${PWD}:/srv/jekyll" \
		${IMAGE_NAME}:latest \
		bundle install

.PHONY: build-image
build-image:
	docker build -t ddanilov1.me .
