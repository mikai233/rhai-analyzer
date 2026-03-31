import "tasks" as tasks;
import "ui";
export const DEFAULT_LIMIT=#{warn:3,critical:5,owners:["ops","dev"]};
fn run(queue,limit){
// hydrate the batch first
let batch=[queue[0],queue[1],queue[2]];
let summary=`jobs=${batch.len()} limit=${limit}`;
if limit>DEFAULT_LIMIT.warn{
tasks::notify(batch,summary);
} else {
for (job,index) in batch {
while index<limit {
if job.active {
ui::render(job.name,index);
break job.name;
} else {
continue;
}
}
}
}
}
