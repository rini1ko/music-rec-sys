# The built front end served by nginx.
#   docker build -t music-rec-web --build-arg VITE_API_URL=http://localhost:3000 .
# VITE_* values are baked into the JavaScript, so anyone who opens the page can
# read them: pass VITE_API_KEY only for a demo on your own machine.

FROM node:24-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
ARG VITE_API_URL=http://localhost:3000
ARG VITE_API_KEY=
ENV VITE_API_URL=$VITE_API_URL \
    VITE_API_KEY=$VITE_API_KEY
RUN npm run build

FROM nginx:1.27-alpine
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html
EXPOSE 80
