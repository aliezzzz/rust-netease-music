import { createApp } from 'vue';
import { createRouter, createWebHashHistory } from 'vue-router';
import App from './App.vue';
import Browse from './views/Browse.vue';
import Settings from './views/Settings.vue';
import './style.css';

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', redirect: '/discover' },
    { path: '/settings', component: Settings },
    { path: '/:kind/:id?', component: Browse },
  ],
});
createApp(App).use(router).mount('#app');
