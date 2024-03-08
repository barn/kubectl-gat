# What is it?

I wanted slightly better output from `kubectl get pods`, so wrote the jankiest wrapper in Rust for no good reason to give me information on the images in use, and a rough run down for the security options being tried.

<img width="800" alt="screenshot of kubectl-gat pods" src="https://github.com/barn/kubectl-gat/assets/39111/b3c8dbeb-5737-408a-b9bd-add72387d627">

# Couldn't you do this with gotemplate/`yq`

yes, but frankly I'd rather write bad Rust than ever use gotemplate.
Kuberninis `get pods` output comes from a table the server returns (apparently), so parsing the JSON from `kubectl get pods -o json` frankly seemed easier.

# This is awful?

Yes this is maybe the worst Rust code you've ever seen, but this is how I'm learning so I don't really care.

# It doesn't work

Yes, that's very possible.
